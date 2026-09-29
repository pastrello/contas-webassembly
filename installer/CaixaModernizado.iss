; Compilar por build-installer.cmd. O AppId deve permanecer estável nas atualizações.
#ifndef AppVersion
  #error AppVersion não informado.
#endif
#ifndef PublishDir
  #error PublishDir não informado.
#endif
#ifndef InstallerOutputDir
  #error InstallerOutputDir não informado.
#endif
#ifndef InstallerAppId
  #define InstallerAppId "CaixaModernizado.Desktop"
#endif

[Setup]
AppId={#InstallerAppId}
AppName=Caixa Modernizado
AppVersion={#AppVersion}
AppPublisher=Caixa Modernizado
AppPublisherURL=https://github.com/pastrello/contas
AppSupportURL=https://github.com/pastrello/contas/issues
VersionInfoVersion={#AppVersion}
DefaultDirName={localappdata}\Programs\CaixaModernizado
DefaultGroupName=Caixa Modernizado
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.17763
DisableProgramGroupPage=yes
DisableDirPage=yes
UsePreviousAppDir=yes
UsePreviousTasks=yes
UninstallDisplayIcon={app}\Caixa.WinForms.exe
AppMutex=Local\CaixaModernizado.SingleInstance
SetupMutex=Local\CaixaModernizado.Installer
; Solicita fechamento pelo mutex; nunca força o encerramento de edições abertas.
CloseApplications=no
RestartApplications=no
WizardStyle=modern
Compression=lzma2
SolidCompression=yes
OutputDir={#InstallerOutputDir}
OutputBaseFilename=CaixaModernizado-Setup-{#AppVersion}-win-x64
InfoBeforeFile=Instalacao.txt

[Languages]
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"

[Tasks]
Name: "desktopicon"; Description: "Criar atalho na área de trabalho"; Flags: unchecked

[Files]
; Lista explícita: nenhum banco, backup ou arquivo do usuário entra no instalador.
Source: "{#PublishDir}\Caixa.WinForms.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{userprograms}\Caixa Modernizado"; Filename: "{app}\Caixa.WinForms.exe"; WorkingDir: "{app}"
Name: "{userdesktop}\Caixa Modernizado"; Filename: "{app}\Caixa.WinForms.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[Run]
Filename: "{app}\Caixa.WinForms.exe"; Description: "Abrir o Caixa Modernizado"; Flags: nowait postinstall skipifsilent

; Não há InstallDelete/UninstallDelete: dados e arquivos não instalados são preservados.
[Code]
function InitializeSetup: Boolean;
var
  InstalledVersion: String;
  InstalledPacked, NewPacked: Int64;
begin
  Result := True;
  if RegQueryStringValue(HKCU64,
    'Software\Microsoft\Windows\CurrentVersion\Uninstall\{#InstallerAppId}_is1',
    'DisplayVersion', InstalledVersion) then
  begin
    if StrToVersion(InstalledVersion, InstalledPacked) and
       StrToVersion('{#AppVersion}', NewPacked) then
    begin
      if ComparePackedVersion(InstalledPacked, NewPacked) > 0 then
      begin
        SuppressibleMsgBox('Já existe uma versão mais recente instalada (' +
          InstalledVersion + '). Esta instalação será cancelada.', mbError, MB_OK, IDOK);
        Result := False;
      end;
    end;
  end;
end;
