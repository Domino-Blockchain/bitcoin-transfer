# AWS CLI Setup

See https://docs.aws.amazon.com/cli/latest/userguide/getting-started-quickstart.html

## Install CLI
```sh
curl "https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip" -o "awscliv2.zip"
unzip awscliv2.zip
sudo ./aws/install
aws --version
rm awscliv2.zip
```

## Configure
```sh
aws configure
# AWS Access Key ID [None]: ...
# AWS Secret Access Key [None]: ...
# Default region name [None]: us-east-2
# Default output format [None]: json

cat .aws/credentials
cat .aws/config
```
