# z80 assembler
Containerized version fo sjasmplus, for testing the z80 implementation

## Building
```bash
docker build --tag sjasmplus .
```

## Running
```bash
./sjasmplus foo.asm
```

Then you can use tools like `xxd` to view the output
