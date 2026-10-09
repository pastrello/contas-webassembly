# Contas WebAssembly

Projeto independente para estudar Blazor WebAssembly com C# e uma API ASP.NET Core, usando SQLite no servidor.

## Origem e separação

Derivado de [pastrello/contas](https://github.com/pastrello/contas), preservando seu histórico Git. Esta base inclui as correções locais da versão 1.6.2 e a auditoria de lançamentos de 02/10/2026.

É uma cópia independente, não um fork nativo do GitHub. O projeto desktop e suas releases continuam no repositório original. Nenhum banco, backup, instalador ou arquivo pessoal foi copiado. As releases desktop não são republicadas aqui.

## Estado inicial

O código ainda é a aplicação desktop de referência. Não há cliente WebAssembly nem API implementados nesta etapa. Os modelos, serviços financeiros, repositórios e testes serão a base do estudo.

Arquitetura planejada:

- Caixa.Web: interface Blazor WebAssembly executada no navegador.
- Caixa.Api: endpoints ASP.NET Core no servidor.
- Caixa.Core: regras financeiras e acesso ao SQLite, usados pela API.
- SQLite: banco no servidor; sem acesso direto pelo navegador.

O primeiro exercício será listar contas e saldos por meio de uma API real. A implementação web usará um banco de estudo separado do banco operacional desktop.

## Documentação herdada

Os arquivos em docs e os scripts de build/instalador ainda descrevem a versão desktop. Servem de referência histórica e não representam uma versão web pronta. O instalador herdado mantém a identidade do aplicativo desktop: não o distribua como produto WebAssembly.

- [README original preservado](docs/README-DESKTOP.md)
- [Arquitetura desktop](docs/01-ARQUITETURA.md)
- [Auditoria financeira](docs/10-AUDITORIA-LANCAMENTOS.md)

A versão 1.6.2 nos metadados identifica a base desktop, não uma release web.