# csv2json

A simple program which consumes CSV and outputs a JSON representation.

## Examples
```
drew@dev:~$ echo "aparitions,humans,specialists" >> ./test.csv
drew@dev:~$ echo "shinobu,hitagi,oshino" >> ./test.csv
drew@dev:~$ echo "mayoi,tsubasa,kaiki" >> ./test.csv
drew@dev:~$ cat ./test.csv | csv2json --pretty
{
  "humans": [
    "hitagi",
    "tsubasa"
  ],
  "aparitions": [
    "shinobu",
    "mayoi"
  ],
  "specialists": [
    "oshino",
    "kaiki"
  ]
}
```
