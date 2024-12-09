# Xtract
Xtract (**X**ML **tra**nsformations **c**onfiguration **t**ool) is a simple CLI app written in Rust for filtering, splitting and transforming XML files.


## Functionality

After compilation, the app is executed on the console using the `xtract` command (or whatever package name you chose). The programme expects exactly one original XML file in the corresponding folder as input. The output depends on the settings in [`config.toml`](config/config.toml) (see below). In a typical use case, certain XML elements are filtered out of the original file and written to a RESIDUE file, while the other elements are transformed if necessary and written to separate files according to the splitting definitions.

## Configuration

The core of the app is the configuration file [`config.toml`](config/config.toml), in which the general settings are defined as well as the positive and negative lists of the filter, the splitting definitions and the transformation rules for individual XML elements. Further settings can be made in files [`log4rs.yml`](config/log4rs.yml) and [`message.toml`](config/message.toml) to define the parameters for logging and to manage the translations for the messages. The absolute paths to the configuration files are to be stored in environment variables `CONFIG`, `LOG4RS` and `MSG_CONFIG` in a `.env` file.
