---
name: Bug report
about: Create a report to help us improve
title: ""
labels: bug
assignees: ''

---

**Describe the bug**
A clear and concise description of what the bug is.

**Hardware Information**
- **Laptop Model (DMI string)**: Provide the output of `cat /sys/class/dmi/id/product_name` or `sudo dmidecode -s system-product-name`.
- **Keyboard Type**: White / RGB 1-zone / RGB 3-zone / Per-Key?
- **Are you dual booting Windows?**: If yes, does the native Control Center work there?

**Daemon Logs**
Please attach the debug output of the daemon while reproducing the error.
Stop the systemd service (`sudo systemctl stop tux-daemon`) and run the daemon manually with debugging enabled:
```sh
just daemon-debug
```
*(Copy and paste the logs here, or attach as a text file if very long)*

**To Reproduce**
Steps to reproduce the behavior:
1. Go to '...'
2. Click on '....'
3. Scroll down to '....'
4. See error

**Expected behavior**
A clear and concise description of what you expected to happen.

**Screenshots/GIFs**
If applicable, add screenshots or `asciinema` recordings of the TUI to help explain your problem.

**Additional context**
Add any other context about the problem here.
