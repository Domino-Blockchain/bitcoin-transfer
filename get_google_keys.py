"""
gcloud kms keys list \
    --keyring TestKeyring \
    --location global \
    --format json

gcloud kms keys versions get-public-key 1 \
    --key TestKey12 \
    --keyring TestKeyring \
    --location global \
    --output-file pubkey_TestKey12.pem
"""
import json
import shlex
import subprocess
from pathlib import Path
from subprocess import Popen, PIPE

def exec(cmd):
    return Popen(shlex.split(cmd), stdout=PIPE, stderr=PIPE)

def get_output(process):
    process.wait()
    return json.loads(process.stdout.read().decode())

list_keys = exec('gcloud kms keys list --keyring TestKeyring --location global --format json')
list_keys = get_output(list_keys)
"""
[{
    "createTime": "2024-05-17T13:39:45.138666418Z",
    "destroyScheduledDuration": "2592000s",
    "name": "projects/domichain-archive/locations/global/keyRings/TestKeyring/cryptoKeys/TestKey4",
    "purpose": "ASYMMETRIC_SIGN",
    "versionTemplate": {
      "algorithm": "EC_SIGN_SECP256K1_SHA256",
      "protectionLevel": "HSM"
    }
}]
"""

filtered_keys = []
for key in list_keys:
    if key["purpose"] != "ASYMMETRIC_SIGN":
        continue
    if key["versionTemplate"] != {"algorithm": "EC_SIGN_SECP256K1_SHA256", "protectionLevel": "HSM"}:
        continue
    filtered_keys.append({"createTime": key["createTime"], "name": key["name"]})

filtered_keys.sort(key=lambda v: v["createTime"])

processes = []
key_name_to_path = {}
for key in filtered_keys:
    key_name = key["name"]
    _, name = key_name.rsplit("/", 1)
    path = f"./google_pubkeys/{name}.pem"
    key_name_to_path[key_name] = path
    p = exec(f'gcloud kms keys versions get-public-key 1 --key {name} --keyring TestKeyring --location global --output-file {path}')
    processes.append(p)

for process in processes:
    process.wait()

def process_pubkey(pk: str) -> str:
    pk = pk.strip()
    begin = "-----BEGIN PUBLIC KEY-----"
    end = "-----END PUBLIC KEY-----"
    if pk.startswith(begin):
        pk = pk[len(begin):]
    if pk.endswith(end):
        pk = pk[:-len(end)]
    return pk.replace("\n", "")

for key in filtered_keys:
    key_name = key["name"]
    path = Path(key_name_to_path[key_name])
    pubkey = path.read_text()
    key["publicKey"] = process_pubkey(pubkey)

print(json.dumps(filtered_keys, indent=2))
