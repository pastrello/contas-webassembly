# Validação do ciclo 3 — 14/10/2026

Branch: codex/saas-cycle-3. Workspace Rust: 0.3.0.

## Resultado

- Build da API nativa Windows e frontend wasm32-unknown-unknown: aprovado.
- cargo test --locked: dois testes unitários aprovados; integração ignorada nesse comando por exigir banco descartável.
- Clippy API/all-targets e frontend WASM com -D warnings: aprovado.
- Test-Cycle2.ps1, ampliado para ciclo 3: integração aprovada em PostgreSQL descartável, com duas aplicações das migrações.
- Confirmação de senha ausente/divergente rejeitada; confirmação exigida também para usuário de consulta.
- Contas e históricos isolados por ambiente, códigos normalizados e duplicidades rejeitadas, inclusive entre inativos.
- Valores em centavos: 0,10 + 0,20 = 0,30; valores negativos, rejeição de formato ambíguo/precisão/limite.
- Edição com versão, duas gravações concorrentes (uma aceita, outra 409), rollback de duplicidade, desativação e reativação.
- Consulta sem permissão de escrita; tentativas entre clientes retornam 404; CSRF e tenant_id forjado rejeitados.
- RLS direto, limpeza do contexto da conexão, chave composta de usuário/ambiente e proibição de DELETE nas novas tabelas.
- Test-Smoke.ps1: dez verificações aprovadas no processo HTTP real, incluindo WASM e comportamento com banco indisponível.
- Banco local: backup completo anterior à migração em .dev/backups; comparação dos registros de usuários, ambientes e vínculos confirmou preservação.

## Limites da validação

A conferência visual automatizada permanece indisponível devido ao bloqueio da ferramenta de navegador. Build WASM e testes HTTP não substituem validação visual e interação manual. Recarregar a página e testar as abas Contas, Históricos e Ambiente.

Linux/Docker não foi executado nesta máquina. Lançamentos e saldos de movimentação pertencem ao próximo ciclo. Nenhuma alteração foi feita no código desktop, instalador ou releases neste ciclo.
