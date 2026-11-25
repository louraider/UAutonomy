; Інсталятор «Автономії» для Windows 10 і 11 (Inno Setup 6).
;
; Ставить застосунок для поточного користувача, без прав адміністратора, у
; %LOCALAPPDATA%\Programs\Avtonomiia, створює ярлики в меню «Пуск» і на
; робочому столі та запис у «Програми та компоненти». Повторний запуск новішого
; інсталятора оновлює застосунок на місці: AppId сталий.
;
; Завантажені пакети (карти, енциклопедії, моделі ШІ) лежать окремо, у
; %APPDATA%\org.uautonomy. Під час видалення інсталятор питає, чи прибрати і
; їх; за замовчуванням вони лишаються, бо це гігабайти, які без інтернету не
; завантажити знову.
;
; Збірка (див. ci.yml, крок «Інсталятор»):
;   iscc /DSourceDir=<тека з uautonomy.exe> /DAppVersion=0.1.0.42 /DOutputDir=<куди> avtonomiia.iss

#ifndef SourceDir
  #define SourceDir "..\..\build\windows\x64\runner\Release"
#endif
#ifndef AppVersion
  #define AppVersion "0.1.0.0"
#endif
#ifndef OutputDir
  #define OutputDir "..\..\build\installer"
#endif

[Setup]
AppId={{6B0F3C1E-5A47-4E8B-9C1D-8A2F4E7D3B19}
AppName=Автономія
AppVersion={#AppVersion}
AppVerName=Автономія {#AppVersion}
VersionInfoVersion={#AppVersion}
VersionInfoProductName=Автономія
VersionInfoDescription=Інсталятор «Автономії»
AppPublisher=UAutonomy
AppPublisherURL=https://uautonomy.app
AppSupportURL=https://github.com/louraider/UAutonomy/issues
AppUpdatesURL=https://uautonomy.app
DefaultDirName={autopf}\Avtonomiia
DefaultGroupName=Автономія
DisableProgramGroupPage=yes
DisableDirPage=auto
DisableReadyPage=yes
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
WizardStyle=modern
SetupIconFile=..\runner\resources\app_icon.ico
UninstallDisplayIcon={app}\uautonomy.exe
UninstallDisplayName=Автономія
OutputDir={#OutputDir}
OutputBaseFilename=avtonomiia-windows-setup
Compression=lzma2/ultra64
SolidCompression=yes
CloseApplications=yes
RestartApplications=no
ShowLanguageDialog=no

[Languages]
Name: "uk"; MessagesFile: "compiler:Languages\Ukrainian.isl"

[CustomMessages]
uk.DeleteData=Видалити також завантажені пакети (карти, енциклопедії, моделі ШІ)?%n%nЯкщо збираєтеся встановити «Автономію» знову, натисніть «Ні»: без інтернету їх не завантажити повторно.
uk.LaunchApp=Відкрити «Автономію»

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[InstallDelete]
; Ресурси Flutter замінюємо повністю, щоб після оновлення не лишилося старих.
Type: filesandordirs; Name: "{app}\data\flutter_assets"

[Files]
Source: "{#SourceDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\Автономія"; Filename: "{app}\uautonomy.exe"
Name: "{autodesktop}\Автономія"; Filename: "{app}\uautonomy.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\uautonomy.exe"; Description: "{cm:LaunchApp}"; Flags: nowait postinstall skipifsilent

[Code]
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  DataDir: String;
begin
  if CurUninstallStep = usPostUninstall then
  begin
    DataDir := ExpandConstant('{userappdata}\org.uautonomy');
    if DirExists(DataDir) and not UninstallSilent then
      if MsgBox(CustomMessage('DeleteData'), mbConfirmation, MB_YESNO or MB_DEFBUTTON2) = IDYES then
        DelTree(DataDir, True, True, True);
  end;
end;
