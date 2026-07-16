; Master Installer for Haven Suite (Desktop App + Server Hosting)

[Setup]
AppName=Haven
AppVersion=2.4.0
AppPublisher=Amni
DefaultDirName={userappdata}\HavenSetupTemp
DisableProgramGroupPage=yes
DisableDirPage=yes
Uninstallable=no
Compression=lzma2/ultra64
SolidCompression=yes
OutputDir=dist
OutputBaseFilename=Haven-Full-Installer
PrivilegesRequired=lowest

[Files]
Source: "dist\Haven-Desktop-Setup.exe"; DestDir: "{tmp}"; Flags: ignoreversion
Source: "dist\Haven-Server-Setup.exe"; DestDir: "{tmp}"; Flags: ignoreversion

[Run]
; Run Desktop Installer silently (the NSIS electron-builder output supports /S)
Filename: "{tmp}\Haven-Desktop-Setup.exe"; Parameters: "/S"; StatusMsg: "Installing Haven Desktop App..."

; Ask the user if they want to install the Server hosting
Filename: "{tmp}\Haven-Server-Setup.exe"; Description: "Install Haven Server (Optional - Host your own server)"; Flags: postinstall nowait unchecked
