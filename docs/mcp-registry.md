# MCP Registry publication

`server.json` describes the `dw-mcp` stdio server for the official MCP
Registry under `io.github.joaocodificacoes/deckwright`. The server exposes
seven inspection tools; deck authoring remains a Rust API and skill workflow.

The registry entry uses the Cargo package `dw-mcp` at the exact version in
`server.json`. The crate must be published to crates.io first, and its rendered
README must visibly contain `mcp-name: io.github.joaocodificacoes/deckwright`
for ownership verification. The official Registry supports Cargo packages,
but `cargo install dw-mcp` requires a Rust toolchain on the user's machine.

After the crate is available on crates.io, run the **Publish MCP Registry
entry** GitHub Actions workflow manually. It validates `server.json`, uses
GitHub OIDC to prove ownership of the GitHub namespace, and publishes the
entry. The workflow pins the official publisher download and checks its
SHA-256. Confirm the published entry through the Registry API before claiming
it is listed. Do not run the workflow before crates.io has the matching
version; its package ownership check will fail.

For a version update, change the workspace crate version, `server.json`
`version` and package `version`, publish the crate, then run the workflow.

The source of truth for the package requirements is the official
[MCP Registry Cargo package guide](https://github.com/modelcontextprotocol/registry/blob/main/docs/modelcontextprotocol-io/package-types.mdx)
and [publisher CLI guide](https://github.com/modelcontextprotocol/registry/blob/main/docs/reference/cli/commands.md).
