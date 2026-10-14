# Validação do ciclo 2 — 14/10/2026

Branch `codex/saas-cycle-2`, baseada no commit `61d3687` do ciclo 1. Workspace Rust 0.2.0. Desktop C#/SQLite mantido.

## Verificações executadas

- Build da API Rust/MSVC e do cliente `wasm32-unknown-unknown`, com bindings wasm-bindgen 0.2.105: aprovados.
- Teste unitário de endpoints sem banco/autenticação: aprovado.
- Teste de integração `authentication_and_tenant_isolation`, em PostgreSQL 15.2 descartável: aprovado. É um cenário integrado com múltiplas verificações, não uma contagem de testes independentes.
- Smoke test HTTP real: 10 verificações aprovadas, incluindo conexão PostgreSQL, arquivos JS/WASM, MIME correto, rota inexistente e indisponibilidade do banco sem derrubar liveness.
- Clippy API/testes e cliente WASM com `-D warnings`: sem avisos. Formatação Rust verificada.
- `docker compose config --quiet`: aprovado. A execução Docker/Linux completa não foi validada.
- Start-Dev aplicou a migração no cluster próprio do SaaS e iniciou a API usando `contas_app`, em `127.0.0.1:8080`.
- Usuário confirmou no navegador a exibição dos formulários “Entrar” e “Criar um ambiente”. A automação visual foi bloqueada pela política da ferramenta; o fluxo de cliques/cadastro no navegador não foi validado automaticamente.

## Cobertura do cenário integrado

- Migração executada duas vezes sem reaplicação; papel restrito aceito e superusuário rejeitado na verificação de inicialização.
- Cadastro de dois clientes, normalização de e-mail, rejeição de senha curta e rollback de cadastro duplicado.
- Leitura/alteração do próprio cliente; leitura/alteração cruzadas rejeitadas; tentativa de enviar tenant_id adicional rejeitada pelo esquema JSON.
- CSRF: origem diferente e cabeçalho obrigatório ausente rejeitados.
- Requisições concorrentes dos dois clientes usando pool de uma única conexão, forçando reuso.
- SQL direto sem contexto não vê clientes; com contexto vê apenas seu cliente. Escrita fora do contexto rejeitada por RLS. Chave composta rejeita vínculo com usuário de outro cliente. Rollback limpa o contexto da conexão.
- Administrador cria usuário de consulta; este acessa seu cliente, mas não altera nome nem cria usuários.
- Erros de login genéricos; senha Argon2id e token de sessão não armazenado em texto puro.
- Renovação de token no login, revogação no logout, sessão expirada e usuário desativado.
- Limite de tentativas mantido entre duas instâncias do Router; cadastro público desabilitado quando configurado; cookie Secure no modo seguro.

## Limites desta entrega

Validação de infraestrutura, identidade e isolamento das tabelas atuais. Ainda não há lançamentos financeiros web. Recuperação de senha, confirmação de e-mail, convites, administração completa de usuários, implantação Linux e preparação de produção ficam para etapas posteriores. Não houve migração ou acesso ao banco operacional desktop. Os dados dos testes automatizados ficaram em clusters descartáveis `.dev`, ignorados pelo Git.
