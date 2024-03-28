import json
import shlex
import subprocess
from pprint import pprint
from subprocess import Popen, PIPE

def exec(cmd):
    return Popen(shlex.split(cmd), stdout=PIPE, stderr=PIPE)

list_keys = exec('aws kms list-keys --query "Keys"')
list_aliases = exec("aws kms list-aliases --query \"Aliases[?contains(@.AliasName,'btci_multisig_')]\"")

list_keys.wait()
list_aliases.wait()

list_keys = json.loads(list_keys.stdout.read().decode())
list_aliases = json.loads(list_aliases.stdout.read().decode())

list_keys = {e["KeyId"]: e["KeyArn"] for e in list_keys}
for e in list_aliases:
    e["KeyArn"] = list_keys[e["TargetKeyId"]]

print(json.dumps(list_aliases, indent=2))
