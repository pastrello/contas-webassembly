# Contas SaaS — ciclo 1

Protótipo separado do desktop C#/SQLite. Rust compila o serviço nativo e o cliente WebAssembly; PostgreSQL armazena os futuros dados do SaaS. Nenhum banco desktop é lido ou migrado.

## Escopo entregue

- Workspace Rust com API Axum, cliente WebAssembly e contrato JSON compartilhado.
- Interface em português: o módulo Rust/WASM consulta a API pela mesma origem.
- Conexão PostgreSQL via SQLx/pool. `SELECT 1` verifica o banco de verdade.
- `GET /health/live`: 204 enquanto o processo responde.
- `GET /health/ready`: 204 com banco disponível, 503 se indisponível.
- `GET /api/status`: estado público sem credenciais nem dados de clientes.
- Scripts de build, execução local e teste com PostgreSQL descartável.
- Docker Compose de desenvolvimento como alternativa ao PostgreSQL instalado.

Este ciclo não oferece login, cadastro de clientes ou operações financeiras. Não publicar na internet como produto pronto. Tabelas, migrações, usuário de aplicação com privilégios mínimos e isolamento entre clientes serão implementados juntos no ciclo 2.

## Windows: executar nesta máquina

Na pasta `saas`, em PowerShell:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/Build.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/Start-Dev.ps1
```

Abra http://127.0.0.1:8080. O script usa PostgreSQL 15 já instalado (binários apenas), cria um cluster próprio em `.dev/postgres`, gera uma senha local aleatória e escuta somente em `127.0.0.1:55432`. Cria o banco `contas_saas` nesse cluster. Não usa o serviço PostgreSQL existente nem sua base. Ctrl+C encerra a API e o banco iniciado pelo script. Para outro caminho, informe `-PostgresBin`.

O compilador instalado durante este ciclo fica em `.dev`, sem alteração global do PATH. Os scripts o reconhecem automaticamente. Em outra máquina, instale Rust/MSVC pelos [passos oficiais](https://rust-lang.org/tools/install/) e execute:

```powershell
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.105 --locked
```

`rust-toolchain.toml` fixa Rust 1.90.0; Cargo.lock fixa as dependências. A versão da CLI wasm-bindgen deve corresponder à biblioteca (0.2.105). O cliente usa Rust/web-sys para consultar a API; JavaScript apenas carrega o módulo. Ainda não adotamos framework de interface.

## Docker (alternativa)

Com Docker Desktop em modo Linux iniciado:

```powershell
Copy-Item .env.example .env
docker compose up --build -d
docker compose logs -f app
```

Abra http://127.0.0.1:8080. Use `docker compose down` para parar, preservando o volume. Não execute simultaneamente com Start-Dev, pois ambos usam as mesmas portas. A senha do exemplo é exclusiva de desenvolvimento; se a alterar, mantenha somente letras/números para a URL desse exemplo. Alterar a variável não troca a senha de um volume PostgreSQL já inicializado.

O Compose usa PostgreSQL 17; a validação nativa nesta máquina usa PostgreSQL 15. A construção e execução Docker dependem de daemon ativo e devem ser verificadas separadamente.

## Verificar

```powershell
. ./scripts/Use-Rust.ps1
cargo fmt --all -- --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo clippy --locked -p contas-web --target wasm32-unknown-unknown -- -D warnings
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/Test-Smoke.ps1
```

O smoke test cria banco descartável e portas próprias, inicia a API, verifica disponibilidade e arquivos WASM, derruba o banco e confirma 503 mantendo liveness 204. Encerra seus processos mesmo em falha. Logs e cluster ficam em `.dev/smoke-*` para diagnóstico, fora do Git. Nunca usa bancos operacionais.

## Próximo ciclo

1. Usuários, clientes (tenants) e vínculos com permissões.
2. Login e sessão; resolução do cliente autorizada no servidor.
3. Migrações PostgreSQL, chaves com tenant_id e Row-Level Security.
4. Testes negativos entre dois clientes, inclusive usando conexões reutilizadas do pool.

Depois: contas/históricos, lançamentos, relatórios e implantação. Manter valores em centavos inteiros. Independência dos clientes será lógica, compartilhando infraestrutura; nenhuma promessa de isolamento existe antes dos testes do ciclo 2.
