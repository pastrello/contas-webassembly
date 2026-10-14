# Contas SaaS — ciclo 3

Rust + WebAssembly + PostgreSQL. Projeto de estudo separado do desktop C#/SQLite. Código em `crates/api`, `crates/web`, `crates/contracts`; migrações versionadas em `migrations`.

## Funcionalidades

- Criar um cliente/empresa com seu usuário administrador.
- Entrar com e-mail e senha, recarregar a página mantendo a sessão e sair revogando a sessão no servidor.
- Administrador altera o nome do ambiente e cadastra novos usuários de consulta.
- Usuário de consulta acessa o mesmo ambiente, sem alterar configurações ou cadastrar usuários.
- Cada cadastro de cliente tem dados independentes. O cliente ativo vem da sessão validada no servidor; o navegador não escolhe um `tenant_id` para autorizar uma operação.

Contas, históricos e saldos iniciais estão disponíveis. Lançamentos financeiros ficam para o ciclo 4. Cadastro público é configurável; nesta fase não há confirmação de e-mail, recuperação/troca de senha, convites ou remoção de usuários pela interface. Usar apenas dados de estudo. A política de recuperação de contas e a preparação operacional antecederão a publicação para clientes reais.

## Executar no Windows

Na pasta `saas`:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/Build.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/Start-Dev.ps1
```

Abra **http://127.0.0.1:8080/** (o endereço deve corresponder a PUBLIC_ORIGIN). O script usa somente os binários PostgreSQL instalados, cria seu próprio cluster em `.dev/postgres`, aplica migrações e configura uma conta `contas_app` restrita. Senhas locais aleatórias ficam em arquivos ignorados em `.dev`. Não acessa o serviço/banco PostgreSQL existente ou qualquer SQLite desktop. Ctrl+C encerra o ambiente iniciado pelo script. Passe `-PostgresBin` se os binários estiverem em outro local.

O compilador Rust e o SDK complementar desta máquina estão em `.dev`, sem alterar PATH global. Em outra máquina, instale [Rust e ferramentas C++ oficiais](https://rust-lang.org/tools/install/), execute `rustup target add wasm32-unknown-unknown` e `cargo install wasm-bindgen-cli --version 0.2.105 --locked`. Versão Rust fixada em `rust-toolchain.toml`; dependências fixadas em Cargo.lock.

## Segurança e isolamento implementados

- Senhas com Argon2id e salt aleatório; cálculo fora das tarefas assíncronas, com concorrência limitada.
- Sessão opaca de 256 bits; banco armazena apenas SHA-256 do token. Validade fixa de 12 horas. Login troca o token presente, logout o remove; usuário desativado e vínculo removido deixam de autorizar acessos.
- Cookie HttpOnly e SameSite=Strict. Secure habilitado por padrão. Somente APP_ENV=development permite cookie sem Secure para testes HTTP locais.
- Escritas exigem Origin igual a PUBLIC_ORIGIN e cabeçalho X-Contas-Request. Sem CORS aberto. Respostas não são armazenadas em cache; CSP limita recursos à própria origem.
- Limitação de tentativas compartilhada no PostgreSQL: 10 logins por e-mail/15 minutos, teto global de 500; 100 cadastros globais e 30 novos usuários por cliente/15 minutos. Conta tentativas bem-sucedidas também. É uma proteção inicial, não substitui controles de tráfego de produção.
- RLS obrigatório em tenants, memberships, tenant_settings, accounts e histories. Contexto definido com `set_config(..., true)` em transação, sem vazar ao próximo uso da conexão. Chaves compostas impedem referências entre clientes.
- A API verifica na inicialização que seu papel não é proprietário das tabelas, superusuário, BYPASSRLS, administrador de papéis/bancos ou membro de outro papel. Ela não executa migrações automaticamente.

Tabelas globais de identidade/sessões são acessadas somente pela API, por consultas parametrizadas. RLS protege dados por cliente; não é uma defesa contra comprometimento da credencial do serviço ou do administrador PostgreSQL. A autorização owner/viewer é aplicada na API. Inicialmente, cada usuário cadastrado pertence a um ambiente; o esquema permite evoluir para vários vínculos, mas não há seletor de ambientes nesta etapa.

## Migrações e Linux/Docker

O binário `contas-api --migrate` usa **MIGRATION_DATABASE_URL**, uma conexão administrativa separada. Por padrão cria o papel contas_app sem login. Se APP_DATABASE_PASSWORD for fornecida ao processo de migração (mínimo 16 caracteres), configura seu login com essa senha. O processo normal usa somente **DATABASE_URL** da conta restrita; nunca fornecer credenciais administrativas ao serviço web.

No Linux, o binário trata SIGTERM para encerramento. Docker Compose faz a sequência banco saudável → migração concluída → API. Na pasta `saas`, com Docker Linux ativo:

```powershell
Copy-Item .env.example .env
docker compose up --build -d
```

Se já existe um `.env` do ciclo 1, adicione APP_DATABASE_PASSWORD. Exemplos são exclusivamente locais. Para este Compose, use senhas alfanuméricas/underscore, ou codifique caracteres especiais na URL. Mudar POSTGRES_PASSWORD não altera automaticamente a senha de um volume existente. Não iniciar Compose junto com Start-Dev: compartilham portas. `docker compose down` preserva o volume.

Compose é de desenvolvimento: portas publicadas apenas no loopback, APP_ENV=development, cadastro aberto. Para produção definir PUBLIC_ORIGIN com HTTPS, remover APP_ENV=development, decidir ALLOW_REGISTRATION e configurar proxy, TLS, backup/restauração, monitoração e controles adicionais. A execução Linux/Docker completa ainda precisa ser validada; testes locais usam PostgreSQL 15 e o Compose prevê 17.

## Testes

```powershell
. ./scripts/Use-Rust.ps1
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo clippy --locked -p contas-web --target wasm32-unknown-unknown -- -D warnings
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/Test-Cycle2.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/Test-Smoke.ps1
```

Test-Cycle2 cria cluster descartável, aplica migrações duas vezes (idempotência), usa pool com **uma conexão** para forçar reutilização e testa autenticação, permissões, RLS direto, tentativas entre clientes, chaves compostas, sessão e limitação compartilhada. O teste fica `ignored` no cargo test comum para nunca escolher um banco operacional por engano. O script configura explicitamente as URLs temporárias.

Test-Smoke verifica o processo HTTP real, arquivos WASM, disponibilidade e queda do banco. Ambos param seus processos e deixam logs em `.dev`, fora do Git.

## Cadastros do ciclo 3

- Cadastro de ambiente e de usuário exige confirmação de senha na interface e na API. Login e usuários existentes são preservados.
- Contas: código, número opcional, nome/banco, saldo inicial e situação ativa/inativa.
- Históricos: código, descrição, sentido explícito Entrada (+1) ou Saída (-1), e situação.
- Administrador cria, edita, desativa e reativa. Usuários de consulta somente visualizam. Códigos são exclusivos por ambiente, inclusive inativos; normalizados em maiúsculas, com zero à esquerda quando têm um caractere.
- Valores são enviados como texto decimal e persistidos em centavos inteiros. Aceitam sinal negativo, vírgula ou ponto decimal, sem separador de milhar, com até duas casas e limite de ±90.000.000.000,00 por conta. A soma usa inteiro de 128 bits e inclui somente contas ativas; não representa saldo de movimentações.
- Edição exige a versão lida: uma alteração concorrente retorna 409. Descarte a edição e atualize a lista antes de tentar novamente. Não há exclusão física pela API.
- Start-Dev cria backup PostgreSQL em .dev/backups antes de migrar e compara usuários, ambientes e vínculos antes/depois. O backup permanece somente nesta máquina; não é uma rotina de backup de produção.
- Test-Cycle2 mantém seu nome para compatibilidade e agora cobre também o ciclo 3: confirmação de senha, valores exatos, cadastros, concorrência, permissões e isolamento das novas tabelas.

A próxima etapa adicionará lançamentos e suas regras transacionais. Antes disso, alterações de sentido e saldo inicial não precisam conciliar movimentações, pois elas ainda não existem neste módulo.