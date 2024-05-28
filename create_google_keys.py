"""
gcloud kms keys create TestKey12 \
    --keyring TestKeyring \
    --location global \
    --purpose "asymmetric-signing" \
    --default-algorithm "ec-sign-secp256k1-sha256" \
    --protection-level "hsm"
"""

import json
import shlex
import subprocess
import time
from subprocess import Popen, PIPE

def exec(cmd):
    return Popen(shlex.split(cmd), stdout=PIPE, stderr=PIPE)

for n in range(0, 100):
    key_name = f"btci_multisig_google_{n:02}"

    create_key = exec(f'gcloud kms keys create {key_name} --keyring TestKeyring --location global --purpose "asymmetric-signing" --default-algorithm "ec-sign-secp256k1-sha256" --protection-level "hsm"')
    create_key.wait()

    print(f'{n}/100')

    # prevent spam
    time.sleep(1)

print("DONE")
