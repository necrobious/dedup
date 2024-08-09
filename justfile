# use PowerShell instead of sh:
# x86_64-unknown-linux-gnu
#set shell := ["powershell.exe", "-c"]

alias b := build-lambda
alias s := synth 
alias d := deploy

build-lambda:
    #cargo lambda build --release --output-format zip --x86-64
    cargo lambda build --release --output-format zip --arm64

synth: build-lambda
    npx cdk synth

deploy: build-lambda
    npx cdk deploy
