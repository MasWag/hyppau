Utils
=====

This directory contains general utilities related to hyper-pattern-matching.

Related to JSON
---------------

### Files

- `schema.json`: A JSON schema of the JSON files representing an NFAH.
    - The schema can be used to validate the input JSON file, for example, using `check-jsonschema`.
        - Example: `pipx run check-jsonschema --schemafile ./schema.json ../examples/small.json`
    - The schema can be used to generate a document explaining the JSON format, for example, using `json-schema-for-humans`.
        - Example: `pipx run json-schema-for-humans schema.json schema.html`

