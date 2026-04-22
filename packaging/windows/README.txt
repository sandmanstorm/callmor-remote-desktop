Callmor Remote Desktop Agent — Windows
========================================

QUICK INSTALL:

1. Right-click "install.bat" and select "Run as administrator".
   Windows SmartScreen may warn you (unsigned). Click "More info" → "Run anyway".

2. Open "C:\ProgramData\Callmor\agent.conf" with Notepad (as Administrator).
   Paste your MACHINE_ID and AGENT_TOKEN from the Callmor dashboard:
     https://remote.callmor.ai

3. Start the service:
       sc start CallmorAgent

4. To check status:
       sc query CallmorAgent

5. To see logs:
       Event Viewer → Windows Logs → Application

TO UNINSTALL:
   Run uninstall.bat as Administrator.

SUPPORT:
   https://remote.callmor.ai
