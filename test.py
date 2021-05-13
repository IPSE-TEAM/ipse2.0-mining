#
import os

process = os.popen("tasklist | findstr exe").readlines()
for i in process:
    print(i)
    try:
        info = i.split()
        if "chrome" in info[0]:
            j = info[1].strip()
            os.system("taskkill /F /PID {0}".format(j))

        print(j)
    except Exception as e:
        print(e)
print(type(process))
a = os.system("./poc-mining.exe > poc.log 2>&1")

print(a)
