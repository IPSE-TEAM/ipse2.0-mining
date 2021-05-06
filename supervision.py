import os
import time
import getopt
import sys
import os
from pathlib import Path

GIB = 1024 * 1024 * 1024

def kill_process(SupervisionFileName, FileName):
	info = os.popen("ps -ef | grep {0}".format(FileName)).readlines()
	if info:
		for i in info:
			if SupervisionFileName not in i:
				try:
					j = i.split()[1].strip()
					os.system("kill -9 " + j)
					print("杀掉进程! {0}".format(i))
				except Exception as e:
					print("删除进程错误! e = {0}, info = {1}".format(e, i))


def run(SupervisionFileName, FileName, LogMaxSize):
	# 检查5分钟 如果有5条日志以上相同 那么判定挖矿异常

	info = None
	start = None
	count = 0
	config_dir = os.path.dirname(FileName)
	# os.system("cd {0}".format(config_dir))
	os.chdir(config_dir)
	dir_url = r'{0}.log'.format(FileName)
	print("122", config_dir)
	print("222", os.path.abspath("./"))

	while True:
		try:

			log_file_size = os.path.getsize(dir_url)
			print("日志文件的大小为: {0}".format(log_file_size))
			# 如果日志文件大于20Gib 那么重启
			if log_file_size > LogMaxSize * GIB:
				print("日志文件太大， 重启...........")
				kill_process(SupervisionFileName, FileName)
				print("关闭挖矿软件!")
				time.sleep(5)

				os.system(r'./{0} > {1}.log 2>&1 &'.format(FileName, FileName))
				print("启动挖矿软件!")
				count = 0
				continue

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
					kill_process(SupervisionFileName, FileName)
					print("关闭挖矿软件!")
					time.sleep(5)

					os.system(r'{0} > {1}.log 2>&1 &'.format(FileName, FileName))
					print("启动挖矿软件!")

					count = 0


		# 没有日志记录或是没有日志文件 说明没有启动软件
		except Exception as e:
			print("没有启动挖矿软件！", e)
			kill_process(SupervisionFileName, FileName)
			print("关闭挖矿软件!")
			result = os.system(r'{0} > {1}.log 2>&1 &'.format(FileName, FileName))
			print("启动挖矿软件!")
			count = 0

		time.sleep(10)


def stop(FileName, SupervisionFileName):
	info = os.popen("ps -ef | grep {0}".format(FileName)).readlines()#.extend(os.popen("ps -ef | grep supervision.py").readlines())
	info1 = os.popen("ps -ef | grep {0}\.py".format(SupervisionFileName)).readlines()
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
		# 开启挖矿： python3 supervision.py --mining 挖矿软件名称 [--log-max-size 数值(默认值是20)] (Gib为基本单位， 比如数值为1， 代表log文件最大空间允许值是1Gib)
		# 结束挖矿： python3 supervision.py --mining 挖矿软件名称 --stop


	FileName = ""          	# 挖矿软件名称
	LogFileMaxSize = 20                 # 日志文件大小最大允许值(多少Gib)

	SupervisionFileName = Path(__file__).name.split(".")[0]
	opts, args = getopt.getopt(sys.argv[1:], "", ["stop", "mining=", "log-max-size="])
	print(opts)

	# 检查是否有文件参数
	for opt, arg in opts:
		if opt == "--mining" and len(arg) != 0:
			FileName = arg
			break
	else:
		exit("请添加mining参数， 并且值不能为空!")

	# 检查是否有log文件大小限制值 如果输入零则使用默认值
	for opt, arg in opts:
		if opt == "--log-max-size" and int(arg) != 0:
			LogFileMaxSize = int(arg)

			break


	# 检查是否有停止命令 有的话直接停止
	for opt, arg in opts:
		if opt == "--stop":
			stop(FileName, SupervisionFileName)
			exit("停止挖矿")
			break
	else:
		run(SupervisionFileName, FileName, LogFileMaxSize)









