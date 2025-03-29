#!/bin/bash

wd=$(pwd)
pay_request="lnbcrt2m1pju8yyypp5fw792f22sn3fkf7v6s9ts8qqp4pctwrxh2lngsjjd04meyqrqt6sdqqcqzpgxqyz5vqsp5hlfxjuve42lf8ha2unuhta2e3uxr9v37yvr72w7gwm3tllqj56ps9qyyssqjuu0dyg9eny69pcf5nfzax97sx8ewg2dhp05ucr3l3j9dqc7xcw8js7zhw0wz3yg55j8ykkw8hrpv7zvgkwuckhr6q3vsva5y8flf9cqatkpp3"

echo $wd

output=$(./submissions/run.sh "$wd" "$wd/test/input.csv" "$pay_request" 500)

# Check the exit status
if [ $? -eq 0 ]; then
    echo "BUILD CHECK PASSED: this **does not** mean your code is correct, it is just a minimal build check."
else
    echo "ERROR: The script exited with an error."
	echo "Run script output: $output"
fi
