# libmongocrypt setup

https://www.mongodb.com/docs/manual/core/csfle/reference/libmongocrypt/#ubuntu

```sh
sudo sh -c 'curl -s --location https://pgp.mongodb.com/libmongocrypt.asc | gpg --dearmor >/etc/apt/trusted.gpg.d/libmongocrypt.gpg'
echo "deb https://libmongocrypt.s3.amazonaws.com/apt/ubuntu `lsb_release -sc`/libmongocrypt/1.8 universe" | sudo tee /etc/apt/sources.list.d/libmongocrypt.list
sudo apt-get update
sudo apt-get install -y libmongocrypt-dev
```
