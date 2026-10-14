# Validação do ciclo 1 — 13/10/2026

Branch: `codex/saas-rust-cycle-1`, derivada da base desktop auditada no repositório `contas-webassembly`.

## Executado

- Build nativo Rust/MSVC da API: aprovado.
- Build `wasm32-unknown-unknown` e geração dos bindings 0.2.105: aprovados.
- `cargo test --locked`: 1 teste automatizado, com quatro verificações de rotas (liveness independente do banco, readiness/status indisponíveis e rota desconhecida).
- Clippy da API/testes e do cliente WASM com `-D warnings`: sem avisos.
- Smoke test com PostgreSQL 15.2 descartável: 10 verificações aprovadas. Inclui conexão real, arquivos servidos, MIME WASM, rota inexistente e indisponibilidade do banco com liveness preservada.
- Script Start-Dev: criou cluster separado com autenticação SCRAM e banco `contas_saas`; API iniciou em loopback.
- Navegador do aplicativo: exibiu “WebAssembly carregado”, “Contas SaaS · versão 0.1.0” e “PostgreSQL conectado”. Conferência visual da página realizada.

## Ambiente e limites

Rust 1.90.0, Windows x64. O compilador e o SDK complementar ficaram em `.dev`, ignorados pelo Git. O Windows tinha MSVC 2019, mas faltavam bibliotecas/headers completos do SDK; foram usados pacotes oficiais `Microsoft.Windows.SDK.CPP` e `.CPP.x64` 10.0.26100.1 do NuGet, sem instalação global.

O Docker Desktop estava sem daemon ativo. O fluxo Docker completo não foi executado; a alternativa nativa foi compilada e testada. O Compose prevê PostgreSQL 17; os testes acima usaram os binários PostgreSQL 15.2 já disponíveis.

Este é um teste de infraestrutura, não uma auditoria SaaS: autenticação, tabelas de negócio e isolamento entre clientes pertencem ao ciclo 2. O desktop C#/SQLite e seus dados não foram usados nos testes.
