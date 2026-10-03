import json
import os
from pathlib import Path
import subprocess
import tempfile
import threading
import tomllib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


repo = Path(__file__).resolve().parents[1]
binary_dir = Path(os.environ.get("OMGATO_BIN_DIR", repo / "target/release"))
requests = []
state = {"on": 0, "brightness": 40, "temperature": 250}
mac = "00:11:22:33:44:55"


class LightServer(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_GET(self):
        requests.append(("GET", self.path))
        if self.path == "/elgato/accessory-info":
            self.respond({"displayName": "Test light", "macAddress": mac})
        else:
            self.respond({"numberOfLights": 1, "lights": [state]})

    def do_PUT(self):
        requests.append(("PUT", self.path))
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        state.update(body["lights"][0])
        self.respond({"numberOfLights": 1, "lights": [state]})

    def respond(self, body):
        data = json.dumps(body).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)


server = ThreadingHTTPServer(("127.0.0.1", 0), LightServer)
worker = threading.Thread(target=server.serve_forever, daemon=True)
worker.start()
try:
    with tempfile.TemporaryDirectory(prefix="omgato-light-checks-") as temporary:
        root = Path(temporary)
        config = root / "config"
        cache = config / "keylight-ctl/lights.toml"
        cache.parent.mkdir(parents=True)
        cache.write_text(
            '[[lights]]\nname = "Test light"\nip = "127.0.0.2"\n'
            f'port = {server.server_port}\nmac = "{mac}"\n'
        )
        bin_dir = root / "bin"
        bin_dir.mkdir()
        discovery_log = root / "discovery.log"
        discovery = bin_dir / "avahi-browse"
        discovery.write_text(
            "#!/bin/sh\n"
            'printf "scan\\n" >> "$DISCOVERY_LOG"\n'
            "printf '%s\\n' '=;lo;IPv4;Test light;_elg._tcp;local;"
            f'test.local;127.0.0.1;{server.server_port};"id={mac}"' + "'\n"
        )
        discovery.chmod(0o700)
        env = os.environ | {
            "HOME": str(root),
            "XDG_CONFIG_HOME": str(config),
            "XDG_STATE_HOME": str(root / "state"),
            "XDG_DATA_HOME": str(root / "data"),
            "XDG_RUNTIME_DIR": str(root),
            "PATH": f"{bin_dir}:{binary_dir}:/usr/bin",
            "DISCOVERY_LOG": str(discovery_log),
            "NO_PROXY": "127.0.0.1,127.0.0.2",
            "no_proxy": "127.0.0.1,127.0.0.2",
        }

        def run(tool, *args):
            result = subprocess.run(
                [str(binary_dir / tool), *args], env=env,
                capture_output=True, text=True, timeout=15, check=True,
            )
            return json.loads(result.stdout)

        for _ in range(3):
            document = run("omgato-panel", "--lights-only", "--skip-lights")
            assert "lights" not in document
        assert not requests and not discovery_log.exists()

        rows = run("keylight-ctl", "--json", "ls")
        assert rows[0]["reachable"] is False
        assert not discovery_log.exists()
        assert tomllib.loads(cache.read_text())["lights"][0]["ip"] == "127.0.0.2"

        rows = run("keylight-ctl", "--json", "on", mac)
        assert rows[0]["reachable"] and rows[0]["on"]
        assert discovery_log.read_text().splitlines() == ["scan"]
        assert tomllib.loads(cache.read_text())["lights"][0]["ip"] == "127.0.0.1"
        document = run("omgato-panel", "--lights-only")
        assert document["lights"][0]["on"]
        history = root / "state/omgato-panel/history.json"
        snapshot = history.read_bytes()
        count = len(requests)
        for _ in range(3):
            assert "lights" not in run("omgato-panel", "--lights-only", "--skip-lights")
        assert len(requests) == count
        assert history.read_bytes() == snapshot

        rows = run("keylight-ctl", "--json", "off", mac)
        assert not rows[0]["on"] and rows[0]["reachable"]
        assert discovery_log.read_text().splitlines() == ["scan"]
        print("PASS: idle status stays offline; explicit power recovers DHCP and preserves history")
finally:
    server.shutdown()
    worker.join()
    server.server_close()
