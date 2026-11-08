"""Gate (ADR-2610281500 §2): in a real pty, pressing Enter on a profile in the
menu leaves the command line in the terminal's input, so the shell reads it.
Run: python3 -I tests/typed_launch.py <ccs-binary>. Exit 0 = the shell got it."""
import os, pty, select, sys, tempfile, time

ccs = os.path.abspath(sys.argv[1])
d = tempfile.mkdtemp()
home = os.path.join(d, "home")
os.makedirs(home)
env = dict(os.environ, CCS_HOME=home, TERM="xterm", HOME=d, SHELL="/bin/sh")
env.pop("CCS_PROFILE", None)
env.pop("HISTFILE", None)
os.system(f"CCS_HOME='{home}' HOME='{d}' '{ccs}' new work </dev/null >/dev/null 2>&1")
os.system(f"CCS_HOME='{home}' HOME='{d}' '{ccs}' default work </dev/null >/dev/null 2>&1")

pid, fd = pty.fork()
if pid == 0:
    # The shell reads what ccs typed, as an interactive shell would.
    os.execve("/bin/sh", ["sh", "-c", f"'{ccs}'; IFS= read -r line; echo \"SHELL-GOT:[$line]\""], env)

def drain(seconds):
    out, end = b"", time.time() + seconds
    while time.time() < end:
        if select.select([fd], [], [], 0.2)[0]:
            try:
                chunk = os.read(fd, 65536)
            except OSError:
                break
            if not chunk:
                break
            out += chunk
    return out

seen = drain(2.0)                 # let the menu draw
os.write(fd, b"\r")               # Enter on the first profile row
seen += drain(4.0)
text = seen.decode(errors="replace")
ok = "SHELL-GOT:[ccs work]" in text
print("PASS" if ok else "FAIL")
if not ok:
    print(text[-800:])
sys.exit(0 if ok else 1)
