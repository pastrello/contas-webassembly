# Instalação e distribuição

## Usar o instalador

Execute `dist/CaixaModernizado-Setup-1.6.1-win-x64.exe` no Windows com suporte a aplicativos x64.
O assistente está em português brasileiro. A instalação é por usuário, sem solicitar
administrador, e inclui o .NET e as dependências da aplicação.

O executável é instalado em `%LOCALAPPDATA%\Programs\CaixaModernizado`.
Há um atalho no menu Iniciar e uma opção de atalho na área de trabalho.
A tela final permite abrir o programa; a instalação silenciosa não o abre.

O banco continua em `%LOCALAPPDATA%\CaixaModernizado\caixa.db`. O instalador não contém
bancos nem copia, migra ou exclui dados. A desinstalação preserva dados e backups.
Em outro computador, restaure um backup pelo próprio programa depois da instalação.

Para atualizar, feche o programa salvando as edições e execute o novo Setup. A identidade
da instalação é estável: o programa aparece uma única vez nos aplicativos instalados.
Também é possível reinstalar a mesma versão. A instalação de uma versão anterior à
registrada é bloqueada. O instalador usa o mutex existente do aplicativo para solicitar
seu fechamento; não força o encerramento.

Este primeiro Setup não possui assinatura digital do projeto; o Windows pode mostrar
editor desconhecido ou aviso de reputação. A assinatura do compilador Inno Setup não
assina automaticamente o instalador produzido. O SHA-256 ao lado do Setup serve para
conferir a integridade do arquivo, não como assinatura do editor.

## Gerar novas versões

Requisitos na máquina de desenvolvimento:

- .NET 10 SDK;
- Inno Setup 6.3 ou superior (esta versão foi gerada com 7.1.0);
- acesso ao NuGet na primeira publicação para obter os pacotes de runtime Windows x64.

1. Altere `Version` em `Directory.Build.props`, mantendo o formato `major.minor.patch`.
2. Execute os testes de regressão.
3. Execute `build-installer.cmd` na raiz do projeto.
4. Distribua o Setup de `dist/`. O arquivo `.sha256` permite conferir seu hash.

Se o compilador não estiver em um caminho conhecido:

```bat
build-installer.cmd -InnoCompiler "C:\caminho\Inno Setup 7\ISCC.exe"
```

Para gerar offline após as dependências já estarem disponíveis no cache local:

```bat
build-installer.cmd -Offline
```

O modo offline não executa a auditoria online de vulnerabilidades. O script usa caches
do projeto em `.build-codex`, preserva as variáveis do processo chamador e publica em uma
pasta nova a cada execução. A lista de arquivos do instalador é explícita e a geração
falha se a publicação deixar de ser um único executável, evitando omitir dependências
silenciosamente. O instalador da mesma versão em `dist/` é substituído ao gerar novamente.

O compilador portátil desta máquina está em `.build-codex/tools/inno-7.1.0` e não é
versionado. Em outra máquina, instale o compilador oficial ou informe seu caminho.

## Arquivos envolvidos

- `Directory.Build.props`: versão única do produto e metadados da compilação.
- `AboutForm`: mostra a versão do executável.
- `build-installer.cmd`: ponto de entrada para publicar e empacotar.
- `installer/Build-Installer.ps1`: publicação self-contained, compilação Inno Setup e hash.
- `installer/CaixaModernizado.iss`: instalação, atalhos, atualização, desinstalação e bloqueio de downgrade.
- `installer/Instalacao.txt`: informações exibidas pelo assistente.
- `installer/Test-Installer.ps1`: teste isolado do ciclo de instalação.

Não altere `InstallerAppId` (`CaixaModernizado.Desktop`) em versões de produção.
O parâmetro de compilação que permite substituí-lo existe para isolar os testes.

## Testar o instalador

```powershell
.\installer\Test-Installer.ps1 -PublishDir "CAMINHO_DA_PUBLICACAO" -InnoCompiler "CAMINHO_DO_ISCC.exe"
```

O teste compila dois Setups com versões 1.6.0 e 1.6.1 e a mesma carga publicada,
mas usa um AppId aleatório exclusivo. Instala sob `.build-codex/installer-tests`,
sem atalhos e sem executar o Caixa. Cria temporariamente o registro de desinstalação
desse AppId no usuário atual e o remove pela desinstalação.

As verificações cobrem instalação inicial, arquivos completos, atualização, reinstalação,
bloqueio de downgrade, bloqueio por mutex e desinstalação. Uma sentinela com nome `caixa.db`
dentro da pasta de teste confirma que arquivos de dados não são removidos. Ela é apenas
um arquivo descartável: o teste não lê nem escreve o banco operacional.

Os logs e artefatos de teste ficam na pasta ignorada para diagnóstico. O teste não faz
inspeção visual do assistente nem executa a aplicação em uma máquina limpa sem .NET.
