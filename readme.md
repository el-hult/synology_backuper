# Synology backupper

Send off a single file to be backed up on a Synology NAS.

Reads the file `config.json` in the same directory as the script. The file should contain the following:

```json
{
    "host": "my.domain.com",
    "port": 3000,
    "username": "myusername",
    "password": "mypassword",
    "share_name": "my_backup",
    "filename": "path/to/file.foo",
}

The program then connects to the Synology NAS at `my.domain.com:3000`, logs in with the user `myusername` and sends the file `path/to/file.foo` to the share `my_backup`, but under the filename `file_YYMMDD_HHMMSS.foo` where `YYMMDD_HHMMSS` is the current date and time.