
import time
import getopt
import sys
import math
from sys import exit
import os
from pathlib import Path
import yaml
import schedule
import smtplib
from email.mime.text import  MIMEText
from email.header import Header
from substrateinterface import SubstrateInterface, Keypair
from substrateinterface.exceptions import SubstrateRequestException
from scalecodec.type_registry import load_type_registry_file

Mib = 1024 * 1024
Gib = 1024 * 1024 * 1024
DOLLARS = 100000000000000
DAY = 5 * 60 * 24
# ProbabilityDeviationValue = 50 / 100

import smtplib
from email.mime.text import  MIMEText
from email.header import Header


def kill_process(SupervisionFileName, FileName):

	process = os.popen("tasklist | findstr {0}".format(FileName)).readlines()
	if process:
		for i in process:
			if SupervisionFileName not in i:
				try:
					info = i.split()
					j = info[1].strip()
					os.system("taskkill /F /PID {0}".format(j))
					print("kill process success:{0}".format(info[0].strip()))
				except Exception as e:
					print("kill process failed:",e)


def check_log_file(LogFileName):
	log_file_size = os.path.getsize(LogFileName)
	print("log file size is: {0}".format(log_file_size))
	if log_file_size > LogFileMaxSize * Mib:
		os.system("del /f/s/q {0}".format(LogFileName))
		# 这里要重启的原因是：可能会因为断网而日志急剧增大
		start(FileName, SupervisionFileName)


def job():

	# 跳转到新的文件夹
	new_dir = os.path.dirname(FileName)
	if len(new_dir) != 0:
		os.chdir(new_dir)

	log_file = r'{0}.log'.format(FileName)

	# 检查日志文件 如果太大 那就删除日志文件
	check_log_file(log_file)

	# 日志信息（日志文件最后一条数据)
	log_info = None
	# 上一条日志的信息
	last_log = None
	same_count = 0
	stop_loop = False

	# 循环检查10次数据
	for i in range(10):
		try:
			with open(log_file, "r", encoding="utf-8") as f:
				#  获取u最后一条日志数据
				file = f.readlines()
				log_info = file[-1]
				print("log info: {0}".format(log_info))
				start_logs = file[:100]
				log_infos = file[-50:]
				del file

				# # 启动时候就没有手续费 可以直接退出
				# for start_log in start_logs:
				# 	if "fees" in start_log:
				# 		exit("Inability to pay some fees.")

				# 日志不同 说明正常 反之需要处理
				if log_info != last_log:
					last_log = log_info
					same_count = 0
				else:
					same_count += 1

				# 如果有异常 那么就重新启动
				if (same_count >= 5):

					print("warn: mining abnormal. Now restart mining, and please wait a moment.")
					start(FileName, SupervisionFileName)
					break

				# 如果日志中有报错信息或是手续费不足 那么退出
				for log in log_infos:
					# print("log:", log)
					if ("fees" in log) or ("Err" in log) or( "err" in log):
						print("Inability to pay some fees or some error occur.")
						start(FileName, SupervisionFileName)
						stop_loop = True
						break

		except Exception as e:
			print("warn: log file does not exists. Now restart mining, and please wait a moment.", e)
			start(FileName, SupervisionFileName)
			break

		if stop_loop:
			break

		# 每5秒钟去检查一次日志
		time.sleep(5)


def start(FileName, SupervisionFileName):
	# 改变当前路径
	new_dir = os.path.dirname(FileName)
	if len(new_dir) != 0:
		os.chdir(new_dir)
	# print("hahahahhahaha")
	kill_process(SupervisionFileName, FileName)
	print("stop mining success!")
	time.sleep(5)
	print("启动命令是: {0}".format(FileName))
	# os.system(r'./{0} > {1}.log 2>&1 &'.format(FileName, FileName))
	result= os.system(r'start /b {0} > {1}.log 2>&1 &'.format(FileName, FileName))
	# print("result: {0}", result)
	if result == 0:
		print("start mining success!")
	else:
		print('start mining failed!')



def stop(FileName, SupervisionFileName):
	process = os.popen("tasklist | findstr {0}".format(FileName)).readlines()
	process1 = os.popen("tasklist | findstr {0}".format(SupervisionFileName)).readlines()
	process.extend(process1)
	if process:
		for i in process:

			try:
				info = i.split()
				j = info[1].strip()
				os.system("taskkill /F /PID {0}".format(j))
				print("杀掉程序: {0}".format(info[0].strip()))
			except Exception as e:
				print("杀进程失败:", e)
	exit("挖矿软件关闭!")


def first_start():

	global FileName
	global LogFileMaxSize
	global StopMining
	opts, args = getopt.getopt(sys.argv[1:], "", ["stop", "mining=", "log-max-size="])
	print(opts)

	# 检查是否有文件参数
	for opt, arg in opts:
		if opt == "--mining" and len(arg) != 0 and "--" not in arg:
			FileName = arg
			break
	else:
		exit("please add '--mining' in your command line, and the value can not empty!")

	# 检查是否有log文件大小限制值 如果输入零则使用默认值
	for opt, arg in opts:
		if opt == "--log-max-size" and int(arg) != 0:
			LogFileMaxSize = int(arg)
			break

	# 检查是否有停止命令 有的话直接停止
	for opt, arg in opts:
		if opt == "--stop":
			stop(FileName, SupervisionFileName)
			StopMining = True
			exit("stop mining!")
			break
	else:
		start(FileName, SupervisionFileName)

if __name__ == "__main__":
	# 监控节点 放在与挖矿软件相同的文件夹中

	# 使用方法：
		# 开启挖矿： python3 supervision.py --mining 挖矿软件名称 [--log-max-size 数值(默认值是20)] (Mib为基本单位， 比如数值为1， 代表log文件最大空间允许值是1Mib)
		# 结束挖矿： python3 supervision.py --mining 挖矿软件名称 --stop

	FileName = ""  # 挖矿软件名称
	LogFileMaxSize = 100  # 日志文件大小最大允许值(多少Mib)
	SupervisionFileName = Path(__file__).name.split(".")[0]
	StopMining = False

	# 检查命令行参数， 并启动挖矿
	first_start()

	if not StopMining:
		# 每5分钟去执行一次(检查日志文件)
		schedule.every(5).minutes.do(job)

		while True:
			schedule.run_pending()












