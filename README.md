# XtracT
XtracT (**X**ML **tra**nsformer with **c**onfiguration **T**OML) is a simple CLI application for filtering, splitting and transforming XML files. Its source code depends on various excellent Rust libraries developed by others listed in the [Cargo.toml](Cargo.toml), but it makes especially heavy use of the awesome [quick_xml](https://github.com/tafia/quick-xml) and [evalexpr](https://github.com/ISibboI/evalexpr) crates. Furthermore, as the expanded name of the application suggests, it is the [TOML](https://github.com/toml-lang/toml) specification that makes it very easy to customise the app to your own needs.

## Functionality

After compilation, the program is executed by running the `xtract` command (or whatever package name you chose in your [Cargo.toml](Cargo.toml)). The program expects exactly one original XML file in the corresponding folder as input. The output depends largely on the settings in the TOML configuration file (see below). In a typical use case, certain XML elements are filtered out of the original file and written to a residual file, while the other elements are transformed if necessary and written to separate files according to the splitting definitions.

## Configuration

The real core of XtracT is the TOML configuration file defining the general settings as well as the positive and negative lists of the filter, the splitting definitions and the transformation rules for individual XML elements. The general structure of this file can be seen in the example file [config.toml](config/config.toml). The mandatory entries are the following:

### element

The **`element`** is a string defining the XML path or the level, so to speak, at which the input file is filtered and split. In the example file all `invoice` elements are sent through the filter before they are distributed to different files according to the splitting definitions. XML nodes above the filter and split level are distributed to all split files.

### filter

The **`filter`** consists of an `allowlist` and a `blocklist`. These lists consist of key-value pairs where the key is a string representing an XML element and the value is a list of strings that are allowed or blocked respectively. In addition to unique strings, you can also use regular expressions as values, although lookarounds are not possible.

The allowlist and the blocklist may be empty. If non-empty their elements must be descendants of the aforementioned filter and split level element. In the example file the entries in the allowlist and in the blocklist define the values the subelements of the `invoice` element must have or must not have in order to pass the filter. The `invoice` elements that don't pass the filter will be collected in a special file whose prefix is defined in the `residue` field of the **`filter`**.

### split

In some use cases the original XML file has to be split into different partial files depending on the respective content of the aforementioned filter and split element. So for certain values of subelements of the filter and split level some sort of labels can be defined in the **`split`** settings.

The labels form the prefixes of the file names of those partial files. For instance all `invoice` elements in the example file whose `invoice_owner` is the "Happy Owner" will be written into a separate file prefixed with the label "LIB001" followed by the original file name and the timestamp of the complete split file. The three parts of the output file name (prefix, original name, timestamp) are connected by underscores. If the boolean `declaration` field is set to `true`, any XML declaration will be written to all split files.

The `default` field of the **`split`** settings defines the prefix of a residual file analogous to the `residue` prefix of the filter. To stay with the example file, all `invoice` elements that pass the filter but miss some split label will be collected in a special file whose prefix is defined in the `default` field.

### transformations

In addition to filtering and splitting, XtracT offers the option of using transformation rules to change certain text nodes in the input file and to delete individual XML elements or add new elements. Entries of the **`transformations`** type have the following structure:
- `target`: the element whose value is to be adjusted or inside/after which new customised elements are to be inserted, depending on the `nodes` property of the transformation rule (see below).
- `keep`: a boolean field defaulting to `true`; if set to `false`, the `target` element and all its descendants will be removed from the output XML regardless of all other settings in the given transformation rule.
- `value`: the new text value of the `target` or of the newly created element. The `value` is either a string literal or the result of the evaluation of an expression. The latter must be a valid expression of the [evalexpr](https://github.com/ISibboI/evalexpr) scripting language.
- `nodes`: new XML nodes that will be created; if specified, instead of the `target` element, the innermost of the newly created nodes will contain the `value` as a text node; there are two different places where the new elements can be inserted: with the `append` keyword they are appended after the `target` element, with the `insert` keyword they are inserted immediately before the end tag of the `target`.
- `source.datafields` and `source.literals`: if the new `value` is computed from an expression containing variables, those variables must either be initialised with values from other XML elements or with literal values. The former are defined in the `datafields` list and the latter in the `literals` list. Please note that  in the current version of XtracT there is an important restriction regarding the `datafields` nodes in that they must not follow after the `target` node in the original XML; otherwise the `value` expression cannot be evaluated when the `target` node is read in.
- `preconditions`: while the `value` can depend on the values of other elements according to (nested) if-then-else expressions, with the `preconditions` field you can also state conditions for the application of the transformation rule as such, depending on the existence of certain other XML elements. With the `existing` keyword you indicate that the rule should only be applied if all child elements specified in the corresponding list occured between the opening and closing tag of the `target`; with the `missing` keyword you indicate that the rule should only be applied if none of the child elements specified in the corresponding list occured between the opening and closing tag of the `target`. If both `existing` and `missing` elements are specified, the two conditions will be linked by logical conjunction.
- `parameters`: a list of parameters that control the behaviour of the transformation rule. In the current version of XtracT, the only permitted parameter is the number of `decimal_places` in numerical values.

### uploads

After applying the filters, the splitting specifications and the transformation rules, the split files are automatically stored in the history folder.

Additionally, selected split files can be copied to remote servers. Corresponding upload scenarios are defined in the **`uploads`** section. The current version of XtracT supports two different transfer protocols: SFTP and SCP. So the `protocol` field of an `upload` procedure can take the corresponding strings "SFTP" or "SCP".

The rest of the **`uploads`** section is largely self-explanatory with the exception of the `include` and `exclude` fields. These are lists containing the prefixes of the file names that are to be transferred to the remote server or, conversely, excluded from the transfer. So, depending on the use case, the user will normally either decide to keep a positive list of all files to be transferred or a negative list of the files to be withheld. If both lists are empty, all transformed files except the filter `residue` and the split `default` will be uploaded.

### settings

The general **`settings`** include the following entries:
- `lang`: the language setting for the info, warn and error messages in the [log file](logs/transformer.log) as well as in the email notifications. Translations are provided by the [message.toml](config/message.toml).
- `history_size`: a numerical field setting the history storage period in days.
- `consistency_check`: a boolean field indicating whether the filter and split settings shall be checked for consistency. The aim of the consistency check is to prevent conflicting values in the `allowlist` and the `blocklist` of the filter as well as inconsistencies regarding the interaction of the filter and split settings that could lead to undesirable results in the output files.
- `inconsistency_notification`: a boolean field indicating whether users shall be notified of possible inconsistency warnings by email.
- `dirs`: a list indicating the paths to the local storage locations. The XML file in the `original` directory is filtered, transformed and split into separate files that are temporarily stored in the `tranformed` directory before they are moved to the date-related subfolder in the `history` directory.
- `timeformats`: timestamp formats for the `history` subfolders and for the names of the transformed XML files.
- `email`: settings of the SMTP server and details of the message dispatch.

## Logging and messages

Further settings can be made in files [log4rs.yml](config/log4rs.yml) and [message.toml](config/message.toml) to define the parameters for logging and to manage the translations for the messages. Please note that the message headings themselves (i. e. the keywords in square brackets in the [message.toml](config/message.toml)) are fixed, while any other languages can be added to the translations.

## Environment variables

The absolute paths to the aforementioned configuration files are to be stored in the environment variables `CONFIG`, `LOG4RS` and `MSG_CONFIG` in a *.env* file located in the home directory of the user owning the `xtract` binary. The required *.env* file thus looks as follows:

```
# Mandatory environment variables
LOG4RS="/absolute/path/to/your/xtract/config/log4rs.yml"
CONFIG="/absolute/path/to/your/xtract/config/config.toml"
MSG_CONFIG="/absolute/path/to/your/xtract/config/message.toml"
```

If the `auth` field in the email configuration is set to `true` because your SMTP server requires user authentication, please set the two additional variables `SMTP_USER` and `SMTP_PW` in the *.env* file.
