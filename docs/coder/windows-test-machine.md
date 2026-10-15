# Set up a Windows machine for remote Coder testing (SSH)

For an agent (or person) on the **Windows** computer. When done, the owner's
Mac can run and debug Coder on this machine over SSH, e.g. to find why the
OpenCode harness doesn't start on Windows.

The OpenAgents host (`openagents computer exec|shell|screenshot`) runs on
macOS and Linux only for now, so Windows uses Windows' own OpenSSH server.

## Steps (PowerShell, **Run as Administrator**)

1. Turn on the SSH server and start it at boot:

   ```powershell
   Add-WindowsCapability -Online -Name OpenSSH.Server~~~~0.0.1.0
   Start-Service sshd
   Set-Service -Name sshd -StartupType Automatic
   ```

   Windows adds the firewall rule `OpenSSH-Server-In-TCP` (port 22). If it
   is missing:

   ```powershell
   New-NetFirewallRule -Name sshd -DisplayName 'OpenSSH Server (sshd)' -Enabled True -Direction Inbound -Protocol TCP -Action Allow -LocalPort 22
   ```

2. Authorize the owner's Mac key. For an account in the Administrators group
   the key goes in the shared admin file (`~/.ssh/authorized_keys` is ignored
   for admins):

   ```powershell
   Add-Content -Path C:\ProgramData\ssh\administrators_authorized_keys -Value "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAICXTAp/0iXQwibcAjiIShoAguRpSb2Fg9abHJHn83Ofg "
   icacls C:\ProgramData\ssh\administrators_authorized_keys /inheritance:r /grant "Administrators:F" /grant "SYSTEM:F"
   ```

   For a non-admin account, add the same line to
   `C:\Users\<you>\.ssh\authorized_keys` instead.

3. Make PowerShell the SSH shell (Coder's installer and commands are PowerShell):

   ```powershell
   New-ItemProperty -Path "HKLM:\SOFTWARE\OpenSSH" -Name DefaultShell -Value "C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe" -PropertyType String -Force
   ```

4. Install the current Coder if it isn't installed: `irm https://openagents.com/cli/install.ps1 | iex`

5. Report back to the owner:
   - the machine's IP address (`ipconfig`, the IPv4 address of the active adapter);
   - the Windows username (`whoami`);
   - whether the Mac and this machine are on the same network. If not, install
     Tailscale on both (https://tailscale.com/download) and report the
     Tailscale IP (`tailscale ip -4`) instead.

## Check from the Mac

```sh
ssh USER@ADDRESS "powershell -Command \"coder --version; opencode --version\""
```

Undo: `Stop-Service sshd; Set-Service sshd -StartupType Disabled`, and remove
the key line from `administrators_authorized_keys`.

What to check on each Windows RC: [Windows release-candidate checks](windows-rc-checks.md).
