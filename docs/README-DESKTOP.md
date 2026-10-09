# Contas

Aplicação desktop para controle simples de contas financeiras, lançamentos, conciliação e relatórios.

O projeto é uma reescrita moderna em **C# / .NET 10 / WinForms**, usando **SQLite** como banco local.

## Recursos

- cadastro de contas;
- cadastro de históricos de entrada e saída;
- lançamentos financeiros por conta;
- saldo real e saldo contabilizado/compensado;
- baixa/arquivamento de lançamentos;
- extratos e consultas por período;
- previsão de cheques;
- relatório por histórico;
- consulta avançada de lançamentos;
- exportação de relatórios para PDF e CSV;
- backup compactado e restauração do SQLite;
- manutenção do banco (`quick_check`, `integrity_check`, `optimize` e `VACUUM`);
- prevenção de segunda instância do programa;
- aviso de alterações não salvas;
- publicação self-contained para Windows x64.

## Tecnologia

- .NET 10
- C#
- Windows Forms
- SQLite (`Microsoft.Data.Sqlite`)
- PDFsharp / MigraDoc

## Estrutura

```text
Contas/
├── CaixaModernizado.sln
├── Directory.Build.props
├── build.cmd
├── publish-win-x64.cmd
├── docs/
└── src/
    ├── Caixa.Core/
    │   ├── Data/
    │   ├── Models/
    │   ├── Repositories/
    │   ├── Services/
    │   └── Utilities/
    └── Caixa.WinForms/
        ├── Forms/
        └── Utilities/
```

## Compilação

Requisitos:

- Visual Studio 2026 com **Desenvolvimento para desktop com .NET**; ou
- .NET 10 SDK.

No Visual Studio, abra:

```text
CaixaModernizado.sln
```

Defina `Caixa.WinForms` como projeto de inicialização e compile a solução.

Na linha de comando:

```bat
build.cmd
```

## Publicação para Windows x64

```bat
publish-win-x64.cmd
```

O script gera uma publicação Release self-contained para Windows x64.

## Instalador Windows

Para publicar e gerar o instalador com Inno Setup 6.3 ou superior:

```bat
build-installer.cmd
```

O Setup é gerado em `dist/`, inclui o .NET e instala para o usuário atual.
O mesmo instalador atualiza versões anteriores, preservando o banco e os backups.
A versão é definida em `Directory.Build.props` e também aparece na tela Sobre.
Consulte [Instalação e distribuição](docs/09-INSTALADOR.md) para os detalhes e testes.

## Banco de dados

O banco é criado automaticamente em:

```text
%LOCALAPPDATA%\CaixaModernizado\caixa.db
```

O arquivo de banco **não deve ser versionado no Git**.

## Backup

A aplicação possui backup e restauração próprios. O backup cria um snapshot consistente do SQLite, valida a base e grava um ZIP com o banco e informações de conferência.

## Observação

Este repositório contém apenas a aplicação moderna. Arquivos DBF, executáveis antigos e ferramentas de conversão não fazem parte deste código-fonte.
