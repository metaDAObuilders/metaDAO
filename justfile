test:
    find programs tests sdk | entr -sc '(cd sdk && yarn build) && RUST_LOG= anchor test'

test-no-build:
    find programs tests sdk | entr -sc '(cd sdk && yarn build) && RUST_LOG= anchor test --skip-build'

# build-verifiable autocrat_v0
build-verifiable PROGRAM_NAME:
	solana-verify build --library-name {{ PROGRAM_NAME }} -b ellipsislabs/solana:1.16.10

deploy PROGRAM_NAME CLUSTER:
	solana program deploy --use-rpc -u {{ CLUSTER }} --program-id ./target/deploy/{{ PROGRAM_NAME }}-keypair.json ./target/deploy/{{ PROGRAM_NAME }}.so --with-compute-unit-price 5 --max-sign-attempts 15 && PROGRAM_ID=$(solana-keygen pubkey ./target/deploy/{{ PROGRAM_NAME }}-keypair.json) && anchor idl init --filepath ./target/idl/{{ PROGRAM_NAME }}.json $PROGRAM_ID --provider.cluster {{ CLUSTER }}

upgrade PROGRAM_NAME PROGRAM_ID CLUSTER:
	anchor upgrade ./target/deploy/{{ PROGRAM_NAME }}.so -p {{ PROGRAM_ID }} --provider.cluster {{ CLUSTER }}

upgrade-idl PROGRAM_NAME PROGRAM_ID CLUSTER:
	anchor idl upgrade --filepath ./target/idl/{{ PROGRAM_NAME }}.json {{ PROGRAM_ID }} --provider.cluster {{ CLUSTER }}
	
bankrun:
    (find programs && find tests) | entr -csr 'anchor build -p autocrat && RUST_LOG= yarn run ts-mocha -p ./tsconfig.json -t 1000000 tests/autocrat.ts'

test-amm:
    find programs tests | entr -csr 'anchor build -p amm && RUST_LOG= yarn run ts-mocha -p ./tsconfig.json -t 1000000 tests/amm.ts'

test-amm-logs:
    find programs tests | entr -csr 'anchor build -p amm && yarn run ts-mocha -p ./tsconfig.json -t 1000000 tests/amm.ts'

bankrun-vault:
    (find programs && find tests) | entr -csr 'anchor build -p conditional_vault && RUST_LOG= yarn run ts-mocha -p ./tsconfig.json -t 1000000 tests/conditionalVault.ts'

bankrun-migrator:
    (find programs && find tests) | entr -csr 'anchor build -p autocrat_migrator && RUST_LOG= yarn run ts-mocha -p ./tsconfig.json -t 1000000 tests/migrator.ts'

bankrun-timelock:
   find programs tests sdk | entr -cs '(cd sdk && yarn build) && anchor build -p optimistic_timelock && RUST_LOG= yarn run ts-mocha -p ./tsconfig.json -t 1000000 tests/timelock.ts'


bankrun-vault-logs:
    find programs tests sdk | entr -cs '(cd sdk && yarn build) && anchor build -p timelock && RUST_LOG= yarn run ts-mocha -p ./tsconfig.json -t 1000000 tests/timelock.ts'

bankrun-logs:
    (find programs && find tests) | entr -csr 'anchor build -p autocrat_v0 && yarn run ts-mocha -p ./tsconfig.json -t 1000000 tests/autocratV0.ts'

build-amm:
	(find programs) | entr -s 'anchor build -p amm'

build:
	(find programs) | entr -s 'anchor build'
