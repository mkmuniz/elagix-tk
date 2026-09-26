# Schliffe — Assistente de prompt (planejamento)

Plano para a próxima frente do Schliffe: ajudar quem escreve o prompt a escrevê-lo de forma **mais precisa e menos custosa**, antes que ele chegue à IA. Nada aqui foi implementado ainda — é o roteiro, na mesma linha do [`MILESTONES.md`](MILESTONES.md).

## Por que isso importa

Medição real de uma semana de uso do Claude Code (12 sessões, ~2.400 respostas), pesando cada tipo de token pelo preço:

| Onde o custo está | Parte do custo |
|---|---|
| Releitura da conversa (cache) a cada resposta | ~75% |
| Conteúdo novo entrando na conversa | ~16% |
| Resposta da IA (inclui o "pensamento") | ~9% |

O Schliffe atual corta ruído de **saída de comandos**, que é uma fatia pequena desse total (~0,6% de economia projetada por semana). O que mais pesa é **quanto a conversa cresce** — e isso começa no prompt: um prompt vago faz a IA explorar o projeto inteiro; um log colado inteiro é relido em todas as respostas seguintes; várias tarefas num prompt só alongam a conversa. Um prompt preciso evita tudo isso.

## O que o assistente é (e o que não é)

- **É:** uma camada que olha o prompt **antes de ele ser enviado** e, quando vale a pena, mostra uma dica ou uma versão reescrita — **só para você**, sem entrar na conversa (custo zero para o modelo principal).
- **Não é:** uma troca automática do prompt. O Claude Code não permite que um hook reescreva o prompt; toda melhoria é uma **sugestão que você aprova** e reenvia.
- **Não é:** um corretor ortográfico. O Claude entende erros de digitação sem problema — o que economiza é **precisão** (onde, o quê, resultado esperado, como verificar), não gramática.

## Princípios

1. **Nunca atrapalhar o fluxo.** Dicas só quando há um problema claro; no máximo uma por vez; fácil de desligar (`SCHLIFFE_PROMPT_TIPS=0`).
2. **Custo zero por padrão.** A análise automática é local, por regras, sem chamar IA. A reescrita com IA só roda quando você pede.
3. **Nunca inventar requisitos.** A reescrita reorganiza o que você disse; se faltar informação, ela pergunta em vez de supor.
4. **Fail-open.** Qualquer erro no assistente deixa o prompt seguir normalmente, como se ele não existisse.
5. **Plataformas:** macOS, Linux e WSL (mesmo limite dos outros hooks — Windows nativo não suportado).

## Milestones

- [ ] **M9 — Base do hook de prompt.** Registrar o Schliffe também no evento `UserPromptSubmit` do Claude Code (via `schliffe hook install` / `install.sh`, com backup e idempotente, como o hook atual). Mostrar mensagens **só para o usuário**, confirmando na prática que elas não entram no contexto da IA. Fail-open e rápido (poucos milissegundos quando não há nada a dizer).
  - *Pronto quando:* um prompt qualquer passa sem atraso perceptível, e uma mensagem de teste aparece para o usuário sem aparecer na conversa.

- [ ] **M10 — Detecções locais (sem IA).** Os dois casos de ganho mais claro:
  1. **Conteúdo colado grande** (log, stack trace, JSON enorme): dica para manter só o trecho relevante ou salvar em arquivo e passar o caminho. Opcional: modo rígido que segura o envio nesse caso.
  2. **Conversa longa:** quando o contexto passar de um limite, sugerir `/compact` (em ponto de pausa) ou conversa nova (ao mudar de assunto).
  - Dicas em português ou inglês, conforme o idioma do prompt; intervalo mínimo entre repetições.
  - *Pronto quando:* testes cobrem os dois casos e os falsos positivos óbvios (ex.: "sim, pode fazer" nunca dispara dica).

- [ ] **M11 — Validação técnica da reescrita (spike).** Antes de construir o revisor, confirmar na prática:
  - que o hook consegue **segurar o envio** e mostrar um texto longo (a sugestão) de forma legível;
  - que dá para chamar o **próprio Claude Code em modo não interativo com o Haiku** de dentro do hook, usando o login que o usuário já tem (sem chave de API separada);
  - latência e custo reais por revisão.
  - *Plano B* se o modo não interativo não funcionar dentro do hook: chave de API própria (configurada pelo usuário) ou um comando manual (`schliffe prompt "rascunho"`) em vez do marcador.
  - *Pronto quando:* há números medidos de latência/custo e uma decisão registrada sobre o caminho escolhido.

- [ ] **M12 — Revisor de prompt sob demanda (`??`).** Começar o prompt com `??` faz o Schliffe reescrevê-lo antes do envio:
  - estrutura sugerida: **objetivo**, **onde** (arquivo/tela), **comportamento atual vs. esperado**, **restrições**, **como verificar** — só com o que o rascunho já diz;
  - quando falta algo essencial, a sugestão **aponta a lacuna** ("faltou dizer em qual tela isso acontece") em vez de inventar;
  - mantém o idioma e o tom do usuário; resposta curta;
  - timeout curto; se a IA auxiliar falhar, o envio é liberado com um aviso.
  - *Pronto quando:* a revisão responde em poucos segundos, e um conjunto de rascunhos reais de exemplo gera sugestões que não adicionam requisitos.

- [ ] **M13 — Ajuste com uso real.** Rodar por alguns dias e medir: quantas dicas aparecem, quantas eram úteis, quantas vezes o `??` foi usado e aceito. Com base nisso, decidir se entram as detecções mais arriscadas (prompt vago, várias tarefas num prompt só) e ajustar limites. Registrar no `schliffe stats` o que for mensurável (ex.: tamanho de conteúdo colado evitado).
  - *Pronto quando:* taxa de dicas inúteis baixa o bastante para deixar ligado por padrão (ou decisão explícita de deixar opt-in).

- [ ] **M14 — Documentação e release.** README (como usar o `??`, como desligar), KNOWN_ISSUES (limitações das heurísticas e da reescrita), CHANGELOG e nova versão.

## Fora do escopo

- **Reescrever o prompt automaticamente** — o Claude Code não permite, e mesmo se permitisse seria arriscado mudar o que o usuário quis dizer sem ele ver.
- **Revisão com IA em todo prompt** — deixaria cada envio mais lento e gastaria tokens em prompts que já estão bons; por isso o `??` é sob demanda.
- **Comprimir a resposta da IA** — ela é gerada no servidor e não passa por nenhum mecanismo do Schliffe. Para respostas mais curtas, o caminho é instrução de concisão (ex.: no `CLAUDE.md`), fora deste plano.

## Riscos conhecidos

- **Heurísticas erram:** um prompt curto pode ser perfeitamente claro no contexto da conversa. Por isso M10 começa só pelos casos inequívocos.
- **A reescrita pode interpretar errado** o que o usuário quis dizer — mitigado por ser sempre sugestão, nunca troca.
- **Economia difícil de medir:** o ganho de um prompt melhor é indireto (menos exploração, menos idas e vindas). Só o conteúdo colado evitado é mensurável com precisão.
- **Dependência do Claude Code:** se o formato ou o comportamento dos hooks mudar, o assistente degrada para "não faz nada" (fail-open), sem quebrar o envio.
