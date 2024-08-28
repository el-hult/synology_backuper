# Synology backuper

Back up a single file on a Synology NAS.

Reads the file `config.json` in the same directory as the script. The file should contain the following:

```json
{
    "host": "my.domain.com",
    "port": 3000,
    "username": "myusername",
    "password": "mypassword",
    "share_name": "my_backup",
    "filename": "path/to/local/file.ext",
}
```

The program then connects to the Synology NAS at `my.domain.com:3000`, logs in with the user `myusername` and sends the file `path/to/local/file.ext` to the share `my_backup`.
Before sending it, the program compresses the target into `path/to/local/file.ext.zip`.
The sent file has the name `file.ext_YYMMDD_HHMMSS.zip` (where `YYMMDD_HHMMSS` is the current date and time).
The zip file loiters around after the upload, so you might want to delete it afterwards.