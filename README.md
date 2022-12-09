# SavedInTime

Directories saved just in time.

## CLI

SIT is a simple tool to take simple snapshots of a changing system. This tool guarantees every file is backed up in the
target zip in a stable state
```
Usage: saved_in_time [OPTIONS] --target-directory <target>

Options:
    -l, --log-level <logger>
        Log Level for the application [default: info]
    -t, --target-directory <target>
        The directory to capture in the snapshot
    -o, --output-file <output>
        Output file for the processed directory. The file is contained in a tar.zst format [default: output.tar.zst]
    -i, --iteration-retries <iteration_retries>
        Amount of iterations the visitor will run before giving up on getting a valid snapshot [default: 5]
    -c, --compression-level <compression_level>
        The compression level to use for the output file [default: 3]
    -h, --help
        Print help information
```