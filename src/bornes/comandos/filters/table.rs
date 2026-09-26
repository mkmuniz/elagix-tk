/// Layer A — column-aligned tables (`docker images`, `docker ps`). Docker
/// pads every cell to its column's width, so most of the bytes are spaces.
/// Columns are located from the header (a column starts where a header word
/// follows 2+ spaces), each row is cut at those offsets, and cells are
/// re-joined with ` | ` — cells can contain single spaces ("Up 2 hours",
/// "2 weeks ago"), so plain whitespace wouldn't be unambiguous. Empty cells
/// stay as empty slots, keeping every row aligned with the header.
///
/// Rows that don't line up with the header (a cell starting mid-column)
/// make the whole table pass through untouched (fail-open, business rule 3).
const MAX_ROWS: usize = 50;

pub fn filter(raw: &str) -> String {
    let mut lines = raw.lines().filter(|l| !l.trim().is_empty());
    let Some(header) = lines.next() else {
        return raw.to_string();
    };
    let starts = column_starts(header);
    if starts.len() < 2 {
        return raw.to_string();
    }
    let mut out = vec![cells(header, &starts).join(" | ")];
    for line in lines {
        if !aligned(line, &starts) {
            return raw.to_string();
        }
        out.push(cells(line, &starts).join(" | "));
    }
    let rows = out.len() - 1;
    if rows > MAX_ROWS {
        out.truncate(MAX_ROWS + 1);
        out.push(format!("[+{} lines omitted: more rows]", rows - MAX_ROWS));
    }
    out.join("\n")
}

fn column_starts(header: &str) -> Vec<usize> {
    let bytes = header.as_bytes();
    let mut starts = vec![0];
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b' ' {
            let run_start = i;
            while i < bytes.len() && bytes[i] == b' ' {
                i += 1;
            }
            if i - run_start >= 2 && i < bytes.len() {
                starts.push(i);
            }
        } else {
            i += 1;
        }
    }
    starts
}

/// A row lines up when the character just before each column start is a
/// space (or the row ended), i.e. no cell spills across a column boundary.
fn aligned(line: &str, starts: &[usize]) -> bool {
    starts.iter().skip(1).all(|&s| {
        line.is_char_boundary(s.min(line.len()))
            && (s >= line.len() || line.as_bytes()[s - 1] == b' ')
    })
}

fn cells<'a>(line: &'a str, starts: &[usize]) -> Vec<&'a str> {
    starts
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            let end = starts
                .get(i + 1)
                .copied()
                .unwrap_or(line.len())
                .min(line.len());
            line.get(s.min(line.len())..end).unwrap_or("").trim()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn docker_images_new_format() {
        let raw = "IMAGE                ID             DISK USAGE   CONTENT SIZE   EXTRA\nalpine:3.20          d9e853e87e55       13.7MB         4.17MB        \nmysql:8              0744ee5ef89c       1.12GB          249MB   U    \n";
        assert_eq!(
            filter(raw),
            "IMAGE | ID | DISK USAGE | CONTENT SIZE | EXTRA\nalpine:3.20 | d9e853e87e55 | 13.7MB | 4.17MB | \nmysql:8 | 0744ee5ef89c | 1.12GB | 249MB | U"
        );
    }

    #[test]
    fn cells_with_spaces_stay_whole() {
        let raw = "CONTAINER ID   IMAGE     STATUS         NAMES\nabc123def456   mysql:8   Up 2 hours     db\n";
        assert_eq!(
            filter(raw),
            "CONTAINER ID | IMAGE | STATUS | NAMES\nabc123def456 | mysql:8 | Up 2 hours | db"
        );
    }

    #[test]
    fn misaligned_rows_pass_through() {
        let raw = "NAME   VALUE\nsomething-long-here x\n";
        assert_eq!(filter(raw), raw);
    }
}
