.PHONY: check coverage install-hooks

check:
	bash scripts/check_all.sh

coverage:
	rustup component add llvm-tools-preview
	cargo install --locked cargo-llvm-cov || true
	cargo llvm-cov --workspace --lcov --output-path target/lcov.info --fail-under-lines 80

install-hooks:
	@command -v pre-commit >/dev/null 2>&1 || (echo "pre-commit not found; run 'pip install --user pre-commit' and ensure ~/.local/bin is in PATH" && exit 1)
	./scripts/install-hooks.sh
