; Haven Server Inno Setup Script
[Setup]
AppName=Haven Server
AppVersion=2.4.0
AppPublisher=Amni
DefaultDirName={userappdata}\HavenServer
DefaultGroupName=Haven Server
UninstallDisplayIcon={app}\public\favicon.svg
Compression=lzma2/ultra64
SolidCompression=yes
OutputDir=dist
OutputBaseFilename=Haven-Server-Setup
PrivilegesRequired=lowest
DisableProgramGroupPage=yes
DisableDirPage=no

[Files]
Source: "*"; DestDir: "{app}"; Excludes: "node_modules,dist,.git,.github,.env,haven.db*,certs,uploads,*.exe,master-setup.iss"; Flags: recursesubdirs createallsubdirs ignoreversion

[Icons]
Name: "{group}\Start Haven Server"; Filename: "{app}\Start Haven.bat"; IconFilename: "{app}\public\favicon.svg"
Name: "{group}\Uninstall Haven"; Filename: "{uninstallexe}"
Name: "{commondesktop}\Start Haven Server"; Filename: "{app}\Start Haven.bat"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional icons:"

[Run]
Filename: "{app}\Install Haven.bat"; Description: "Launch Setup Wizard (Installs Node.js & Configures Server)"; Flags: postinstall nowait
