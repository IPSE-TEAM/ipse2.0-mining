import os
import time
import getopt
import sys

def kill_process(FileName):
	info = os.popen("ps -ef | grep {0}".format(FileName)).readlines()
	if info:
		for i in info:
			try:
				j = i.split()[1].strip()
				os.system("kill -9 " + j)
				print("杀掉进程! {0}".format(i))
			except Exception as e:
				print("删除进程错误! e = {0}, info = {1}".format(e, i))


def run(FileName):
	# 检查5分钟 如果有5条日志以上相同 那么判定挖矿异常

	info = None
	start = None
	count = 0
	dir_url = r'./{0}.log'.format(FileName)

	while True:
		try:
			with open(dir_url, "r") as f:

				info = f.readlines()[-1]  # .split()
				print(info)

				if info != start:
					count = 0
					start = info

				else:
					count += 1
					print(count)

				# 卡住5次以上或是出现Error 马上重启
				if (count >= 5) or ("Error" in info) or ("error" in info):
					kill_process(FileName)
					print("关闭挖矿软件!")
					time.sleep(5)

					os.system(r'./{0} > {1}.log 2>&1 &'.format(FileName, FileName))
					print("启动挖矿软件!")

					count = 0


		# 没有日志记录或是没有日志文件 说明没有启动软件
		except Exception as e:
			print("没有启动挖矿软件！")
			kill_process(FileName)
			print("关闭挖矿软件!")
			result = os.system(r'./{0} > {1}.log 2>&1 &'.format(FileName, FileName))
			print("启动挖矿软件!")
			count = 0

		time.sleep(10)


def stop(FileName):
	info = os.popen("ps -ef | grep {0}".format(FileName)).readlines()#.extend(os.popen("ps -ef | grep supervision.py").readlines())
	info1 = os.popen("ps -ef | grep supervision.py").readlines()
	info.extend(info1)
	if info:
		for i in info:
			print(i)
			try:
				j = i.split()[1].strip()
				os.system("kill -9 " + j)
				print("杀掉进程! {0}".format(i))
			except Exception as e:
				print("删除进程错误! e = {0}, info = {1}".format(e, i))


if __name__ == "__main__":
	# 监控节点 放在与挖矿软件相同的文件夹中

	# 使用方法：
		# 开启挖矿： python3 supervision.py --start
		# 结束挖矿： python3 supervision.py --stop

	FileName = "poc-mining"
	opts, args = getopt.getopt(sys.argv[1:], "", ["stop", "start"])
	if len(opts) == 1:
		for opt, arg in opts:
			if opt == "--stop":
				stop(FileName)
			elif opt == "--start":
				run(FileName)
			else:
				exit("终端命令输入错误！ 请再次输入。")
	else:
		exit("终端命令输入错误！ 请再次输入。")








