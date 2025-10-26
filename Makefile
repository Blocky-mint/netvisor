.PHONY: help build test clean format

help:
	@echo "NetVisor Development Commands"
	@echo ""
	@echo "  make setup-db       - Set up database"
	@echo "  make clean-db       - Clean up database"
	@echo "  make clean-daemon       - Remove daemon config file"
	@echo "  make dump-db       - Dump database to /netvisor"
	@echo "  make dev-server     - Start server dev environment"
	@echo "  make dev-server     - Start server dev environment"
	@echo "  make dev-ui         - Start ui"
	@echo "  make dev-daemon     - Start daemon dev environment"
	@echo "  make dev-container  - Start containerized development environment using docker-compose.dev.yml (server + ui + daemon)"
	@echo "  make dev-container-rebuild  - Rebuild and start containerized dev environment"
	@echo "  make dev-container-rebuild-clean  - Rebuild, clean, and start containerized dev environment"
	@echo "  make dev-down       - Stop development containers"
	@echo "  make build          - Build production Docker images (server + ui + daemon)"
	@echo "  make test           - Run all tests"
	@echo "  make lint           - Run all linters"
	@echo "  make format         - Format all code"
	@echo "  make clean          - Clean build artifacts and containers"
	@echo "  make install-dev    - Install local development dependencies"

setup-db:
	@echo "Setting up PostgreSQL..."
	@docker run -d \
		--name netvisor-postgres \
		-e POSTGRES_USER=postgres \
		-e POSTGRES_PASSWORD=password \
		-e POSTGRES_DB=netvisor \
		-p 5432:5432 \
		postgres:17-alpine || echo "Already running"
	@sleep 3
	@echo "PostgreSQL ready at localhost:5432"

clean-db:
	docker stop netvisor-postgres || true
	docker rm netvisor-postgres || true

clean-daemon:
	rm -rf ~/Library/Application\ Support/com.netvisor.daemon

dump-db:
	docker exec -t netvisor-postgres pg_dump -U postgres -d netvisor > ~/dev/netvisor/netvisor.sql  

dev-server:
	@export DATABASE_URL="postgresql://postgres:password@localhost:5432/netvisor" && \
	cd backend && cargo run --bin server -- --log-level debug

dev-daemon:
	cd backend && cargo run --bin daemon -- --server-target 127.0.0.1 --server-port 60072 --log-level debug

dev-ui:
	cd ui && npm run dev

dev-container:
	docker compose -f docker-compose.yml -f docker-compose.dev.yml up

dev-container-rebuild:
	docker compose -f docker-compose.yml -f docker-compose.dev.yml up --build --force-recreate

dev-container-rebuild-clean:
	docker compose -f docker-compose.yml -f docker-compose.dev.yml build --no-cache
	docker compose -f docker-compose.yml -f docker-compose.dev.yml up

dev-down:
	docker compose -f docker-compose.yml -f docker-compose.dev.yml down --volumes --rmi all

build:
	@echo "Building server Docker image..."
	docker compose build
	@echo "Building daemon Docker image..."
	docker build -f backend/Dockerfile.daemon -t mayanayza/netvisor-daemon:latest server/
	@echo "✓ Daemon image built: mayanayza/netvisor-daemon:latest"

test:
	make dev-down
	rm -rf ./data/daemon_config/*
	@export DATABASE_URL="postgresql://postgres:password@localhost:5432/netvisor_test" && \
	cd backend && cargo test -- --nocapture --test-threads=1

format:
	@echo "Formatting Server..."
	cd backend && cargo fmt
	@echo "Formatting UI..."
	cd ui && npm run format
	@echo "All code formatted!"

lint:
	@echo "Linting Server..."
	cd backend && cargo fmt -- --check && cargo clippy --bin server -- -D warnings
	@echo "Linting Daemon..."
	cd backend && cargo clippy --bin daemon -- -D warnings
	@echo "Linting UI..."
	cd ui && npm run lint && npm run format -- --check && npm run check

clean:
	make clean-db
	docker compose down -v
	cd backend && cargo clean
	cd ui && rm -rf node_modules dist build .svelte-kit

install-dev:
	@echo "Installing Rust toolchain..."
	rustup install stable
	rustup component add rustfmt clippy
	@echo "Installing Node.js dependencies..."
	cd ui && npm install
	@echo "Installing cargo-watch for hot reload..."
	cargo install cargo-watch
	@echo "Installing postgresql..."
	brew install postgresql@17
	echo 'export PATH="/opt/homebrew/opt/postgresql@17/bin:$PATH"' >> ~/.zshrc
	brew services start postgresql@17
	source ~/.zshrc
	@echo "Development dependencies installed!"