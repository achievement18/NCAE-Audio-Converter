# NCAE Audio Converter

A native Windows Rust desktop tool for local NCAE effect management, IR inspection and export.

Two independent variants are included: a native WAV/IRS/parameter-JSON application and a multi-format experimental application with a private bundled conversion runtime.

Extract the complete release ZIP and run the main executable. Do not remove runtime or converter when using the experimental variant.

The default generation action backs up and replaces the selected effect. Ordinary replacement does not create an extra export. Explicit exports go to Downloads; generate-only output goes to the application generated folder.

Only selected library effects are previewed. Module tabs are read-only. JSON EQ/PEQ views are settings/model previews rather than measured whole-effect responses; unknown numeric filter types are not inferred.

See [Chinese README](README.md), [build instructions](docs/BUILD.md), [format notes](docs/FORMATS.md), and [third-party notices](THIRD_PARTY_NOTICES.md).

Project-authored code licensing is pending owner confirmation. No new license grant is made by this repository preparation.
