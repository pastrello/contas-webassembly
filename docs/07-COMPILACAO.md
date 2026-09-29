# 07 - Compilação

## Visual Studio

Abra `CaixaModernizado.sln` e use **Compilar > Compilar Solução**.

A primeira compilação restaura automaticamente os pacotes NuGet.

## Linha de comando

Na raiz do projeto:

```bat
build.cmd
```

Ou manualmente:

```bat
dotnet restore CaixaModernizado.sln
dotnet build CaixaModernizado.sln -c Release
```

## Compilação pelo Codex no Windows

Quando o ambiente restrito impede a consulta a `Microsoft SDKs` ou à configuração pessoal do NuGet, execute na raiz:

```bat
build-codex.cmd
```

Para recompilar todos os fontes, use `build-codex.cmd -t:Rebuild`.

O script mantém configurações e caches em `.build-codex/` (ignorada pelo Git), utiliza uma fonte local alimentada pelos arquivos `.nupkg` já existentes em `%USERPROFILE%\.nuget\packages` e informa apenas o nome de exibição da plataforma Windows ao MSBuild. Não altera o framework de destino nem exige administrador. As variáveis de ambiente valem somente durante o script.

Esse caminho funciona offline para as dependências já disponíveis. Dependências novas precisam ser obtidas antes de usá-lo. A auditoria online de vulnerabilidades não é executada nesse modo; use a restauração online normal quando disponível. `build.cmd` continua sendo o fluxo normal conectado à internet.

Validado no Codex com SDK 10.0.401: restauração local e recompilação Release concluídas, com zero erros e os três avisos de nulabilidade já existentes.

## Dependências externas

A aplicação usa:

- `Microsoft.Data.Sqlite` 10.0.11

## Banco de dados

O arquivo não faz parte da solução compilada. É criado na primeira execução em:

`%LOCALAPPDATA%\CaixaModernizado\caixa.db`
