import json
import shlex
import subprocess
from subprocess import Popen, PIPE

def exec(cmd):
    return Popen(shlex.split(cmd), stdout=PIPE, stderr=PIPE)

def get_output(process):
    process.wait()
    return json.loads(process.stdout.read().decode())

list_keys = exec('aws kms list-keys --query "Keys"')
list_aliases = exec("aws kms list-aliases --query \"Aliases[?contains(@.AliasName,'btci_multisig_')]\"")

list_keys = get_output(list_keys)
list_aliases = get_output(list_aliases)

list_keys = {e["KeyId"]: e["KeyArn"] for e in list_keys}
for e in list_aliases:
    e["KeyArn"] = list_keys[e["TargetKeyId"]]

processes = []
for key in list_aliases:
    key_arn = key["KeyArn"]
    p = exec(f'aws kms get-public-key --key-id {key_arn}')
    processes.append(p)

pubkeys = {}
for process in processes:
    output = get_output(process)
    assert output["CustomerMasterKeySpec"] == "ECC_SECG_P256K1"
    assert output["KeySpec"] == "ECC_SECG_P256K1"
    assert output["KeyUsage"] == "SIGN_VERIFY"
    assert output["SigningAlgorithms"] == ["ECDSA_SHA_256"]
    pubkeys[output["KeyId"]] = output["PublicKey"]

for key in list_aliases:
    key["PublicKey"] = pubkeys[key["KeyArn"]]

print(json.dumps(list_aliases, indent=2))
