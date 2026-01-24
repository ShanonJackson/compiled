# Development Guide

This document covers the basic development workflow for the Compiled monorepo.

## Prerequisites

- **Node.js**: 16.x, 18.x, 20.x, or 22.x
- **Yarn**: 1.x (classic)

## Initial Setup

```bash
# Install all dependencies (including workspaces)
yarn install

# Build all packages
yarn build
```

## Project Structure

```
compiled/
├── packages/       # Main npm packages (@compiled/react, @compiled/babel-plugin, etc.)
├── examples/       # Example applications (webpack, parcel, ssr)
├── fixtures/       # Test fixtures
└── test/           # Shared test utilities
```

## Development Workflow

### 1. Making Code Changes

Edit files directly in `packages/<package-name>/src/`. The source code is TypeScript.

### 2. Rebuilding After Changes

After modifying TypeScript code, rebuild to see your changes:

```bash
# Rebuild all packages
yarn build

# Or rebuild incrementally (faster for iteration)
yarn build:cjs    # CommonJS output only
yarn build:esm    # ESM output only
```

### 3. Running Tests

```bash
# Run all tests
yarn test

# Run tests in watch mode (recommended during development)
yarn test:watch

# Run tests for a specific package
yarn test --testPathPattern=packages/css
yarn test --testPathPattern=packages/babel-plugin
yarn test --testPathPattern=packages/react

# Run tests matching a specific test name
yarn test -t "should handle nested selectors"

# Run Parcel-specific integration tests
yarn test:parcel

# Verify package exports work correctly
yarn test:imports
```

### 4. Code Quality

```bash
# Run linting
yarn lint

# Fix lint issues automatically
yarn lint --fix

# Check formatting
yarn prettier:check

# Fix formatting
yarn prettier:write

# Type checking (runs as part of build)
yarn build
```

## Common Development Tasks

### Adding a New Feature

1. Make changes in `packages/<package>/src/`
2. Add/update tests in `packages/<package>/src/__tests__/`
3. Rebuild: `yarn build`
4. Run tests: `yarn test --testPathPattern=packages/<package>`

### Debugging a Test

```bash
# Run a single test file
yarn test packages/css/src/__tests__/transform.test.ts

# Run with verbose output
yarn test --verbose --testPathPattern=packages/css

# Run in watch mode for rapid iteration
yarn test:watch --testPathPattern=packages/css
```

### Testing Changes in Example Apps

```bash
# After rebuilding packages, test in the webpack example
cd examples/webpack-app
yarn start

# Or the parcel example
cd examples/parcel-app
yarn start
```

## Package Dependencies

This is a Yarn Workspaces monorepo. Packages reference each other via workspace protocol:

- Changes to `@compiled/css` affect `@compiled/babel-plugin` and others
- Always rebuild after changing a dependency package
- Run the full test suite before submitting PRs: `yarn test`

## Troubleshooting

### Tests failing after changes?

```bash
# Clear Jest cache and rebuild
yarn build
yarn test --no-cache
```

### Module not found errors?

```bash
# Reinstall dependencies and rebuild
rm -rf node_modules
yarn install
yarn build
```

### Seeing stale code?

The build outputs to `dist/` directories. Make sure to rebuild:

```bash
yarn build
```

## CI Checks

Before submitting a PR, ensure these pass locally:

```bash
yarn lint
yarn prettier:check
yarn build
yarn test
yarn test:imports
yarn test:parcel
```
