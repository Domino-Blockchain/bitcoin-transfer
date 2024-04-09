"""
aws kms create-key \
    --policy file://aws_kms_policy.json \
    --description "Key to sign multisig withdrawal transactions in BTC" \
    --key-usage SIGN_VERIFY \
    --key-spec ECC_SECG_P256K1

aws kms create-alias \
    --alias-name alias/btci_multisig_05 \
    --target-key-id arn:aws:kms:us-east-2:571922870935:key/bca8854f-0c4d-4a79-8f99-f09b1e1ab98b
"""

import json
import shlex
import subprocess
import time
from subprocess import Popen, PIPE

def exec(cmd):
    return Popen(shlex.split(cmd), stdout=PIPE, stderr=PIPE)

def get_output(process):
    process.wait()
    return json.loads(process.stdout.read().decode())

for n in range(0, 100):
    key_name = f"btci_multisig_{n:02}"

    print("KeyName", key_name)

    create_key = exec('aws kms create-key --policy file://aws_kms_policy.json --description "Key to sign multisig withdrawal transactions in BTC" --key-usage SIGN_VERIFY --key-spec ECC_SECG_P256K1')
    create_key = get_output(create_key)
    key_arn = create_key["KeyMetadata"]["Arn"]

    print("KeyArn", key_arn)

    create_alias = exec(f'aws kms create-alias --alias-name alias/{key_name} --target-key-id {key_arn}')
    create_alias.wait()

    print("_")
    time.sleep(1)

print("DONE")
