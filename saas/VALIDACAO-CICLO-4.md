# Validação do ciclo 4 — 14/10/2026

Branch codex/saas-cycle-4; workspace Rust 0.4.0.

## Verificações aprovadas

- Build nativo da API Windows e frontend wasm32-unknown-unknown.
- Três testes unitários: indisponibilidade/autenticação, dinheiro exato e calendário (incluindo bissextos).
- Clippy API/all-targets e frontend WASM com -D warnings.
- Test-Cycle2.ps1: regressão integrada dos ciclos 2, 3 e 4, em PostgreSQL descartável. Migrações aplicadas duas vezes. Os testes não usam o banco local do usuário.
- Ciclo financeiro com quatro conexões: abertura 1000, entrada compensada 200, saída pendente 150 => real 1050, bancário 1200; compensação da saída => ambos 1050.
- Edição de valor, desfazer compensação, mudança de conta/histórico, arquivamento/desarquivamento e cancelamento preservando registro.
- Datas inválidas, zero/negativo, precisão excessiva, valor acima do limite e período invertido rejeitados.
- Duas inclusões simultâneas com mesma chave retornam o mesmo ID, sem duplicar saldo; chave com conteúdo diferente retorna 409. Reenvio após cancelamento não recria o movimento.
- Duas edições simultâneas da mesma versão: uma aceita e outra retorna 409.
- Saldo anterior, limites inclusivos do período, soma de arquivados e exclusão de cancelados.
- Paginação com 201 lançamentos de um centavo: 200 na primeira página, um na segunda, ordem estável na mesma data, sem repetição e totais de 2,01 em ambas.
- Saldo inicial/sentido de cadastros usados protegidos; novos vínculos com conta inativa rejeitados, correção mantendo referência preexistente permitida.
- Usuário de consulta sem escrita; CSRF, tenant_id forjado e referências entre clientes rejeitados.
- RLS direto, contexto de conexão limpo, FKs compostas e ausência de permissão DELETE sobre lançamentos.
- Test-Smoke.ps1: dez verificações do processo HTTP real, incluindo MIME WASM, disponibilidade e queda do banco.
- Banco local atualizado com backup completo anterior. Comparações antes/depois confirmaram preservação de identidades, contas e históricos existentes.

## Limites

A verificação visual automatizada permanece bloqueada pela ferramenta de navegador. Compilação WASM e HTTP não confirmam o comportamento visual/interação manual; recarregue a página e confira a aba Lançamentos.

Linux/Docker e teste de carga não executados. Paginação limita a resposta, mas a totalização ainda lê todos os movimentos da conta no servidor. Cancelamento preserva o registro, não um histórico completo de versões. Importação do SQLite, PDF, integração bancária, transferências automáticas em pares e operações em lote não fazem parte deste ciclo.

O C#/SQLite, instaladores e releases do desktop não foram alterados.
