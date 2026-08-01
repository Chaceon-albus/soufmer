# Runtime Manifest

The runtime manifest is trusted release configuration compiled into the deterministic bootstrap
payload. It selects one compatible private runtime and describes every downloaded artifact with
immutable identity and integrity metadata.

## Required principles

- Use a schema version and a unique runtime version.
- Pin the supported operating system and architecture to Windows x64 for the MVP.
- Pin Python to the 3.11 minor line and install it through the embedded uv build.
- Give every downloaded archive or checkpoint an HTTPS URL, exact byte size, and lowercase
  64-character SHA-256 digest.
- Record the FFmpeg version, provider, archive layout, and license classification.
- Record the model repository, immutable revision, filename, and checkpoint digest.
- Record estimated total download and installed byte counts used by the confirmation UI.
- Never store credentials, mutable branch references, developer-machine absolute paths, or Git/VCS
  dependencies in the manifest.

The build rejects placeholder values in the trusted runtime and vendor manifests. Incomplete
runtime state on disk reports `notInstalled` or `repairRequired`; it is never treated as a valid
substitute for a complete compile-time manifest.

## Activation

1. Acquire the application-wide initialization mutex.
2. Revalidate the active bootstrap and any current runtime state.
3. Create a uniquely named inactive runtime version and stage downloads below the private root.
4. Verify SHA-256 before extraction.
5. Extract with traversal, link, duplicate-path, count, and size protections.
6. Install managed Python and synchronize the worker with the committed lockfile.
7. Run the private runtime self-test.
8. Keep the completed environment in that versioned directory and atomically replace the
   active-state file. The environment is never relocated after uv creates it.

The previous ready runtime stays available until the new runtime has passed self-test.

## Producing release values

Use the exact artifact bytes that will be served to end users. Calculate SHA-256 with a binary-safe
tool, record the source URL and license files, and repeat the calculation after downloading the
artifact from its public release location. Update the manifest, lockfile, notices, and release
checklist together. A clean initialization must be tested without global Python, uv, FFmpeg, or
Git.
