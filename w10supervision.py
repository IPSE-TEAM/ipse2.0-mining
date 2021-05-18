
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


def get_config():
    with open("config.yaml", "r", encoding="utf-8") as yaml_r:
        result = yaml_r.read()
        x = yaml.load(result)
        return x


def email_server(mes):
	config = get_config()
	is_open = config["is_open_email_service"]
	if is_open:
		config = get_config()
		# 第三方 SMTP 服务
		mail_host = config['mail_host']
		mail_user = config['mail_user']
		mail_pass = config['mail_pass']  # 口令
		subject = config["subject"]

		sender = mail_user

		# sender = "TRANSXASK"
		receivers = [mail_user]  # 接收邮件，可设置为你的QQ邮箱或者其他邮箱

		message = MIMEText(mes, 'plain', 'utf-8')  # 邮件内容
		message['From'] = Header("TRANSXASK", 'utf-8')
		message['To'] = Header("挖矿", 'utf-8')

		message['Subject'] = Header(subject, 'utf-8')

		try:
			smtpObj = smtplib.SMTP()
			smtpObj.connect(mail_host, 25)  # 25 为 SMTP 端口号
			smtpObj.login(mail_user, mail_pass)
			smtpObj.sendmail(sender, receivers, message.as_string())
			print("邮件发送成功")

		except smtplib.SMTPException as e:
			print("Error: 无法发送邮件", e)


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
		email_server("{0}日志太大: {1}".format(time.asctime(time.localtime(time.time())), log_file_size))
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
					email_server("{0}挖矿异常， 信息：{1}".format(time.asctime(time.localtime(time.time()))), log_info)
					start(FileName, SupervisionFileName)
					break

				# 如果日志中有报错信息或是手续费不足 那么退出
				for log in log_infos:
					# print("log:", log)
					if ("fees" in log) or ("Err" in log) or( "err" in log):
						print("Inability to pay some fees or some error occur.")
						email_server("{0}挖矿异常， 信息：{1}".format(time.asctime(time.localtime(time.time()))), log_info)
						start(FileName, SupervisionFileName)
						stop_loop = True
						break

		except Exception as e:
			print("warn: log file does not exists. Now restart mining, and please wait a moment.", e)
			email_server("{0}挖矿日志文件异常， 信息：{1}".format(time.asctime(time.localtime(time.time())), e))
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
		email_server("{0},启动挖矿成功!".format(str(time.asctime(time.localtime(time.time())))))
	else:
		print('start mining failed!')
		email_server("{0}启动挖矿失败!".format(str(time.asctime(time.localtime(time.time())))))



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

	# 检查一下链上信息
	check_on_chain()

def get_reward_info():
	config = get_config()
	url = config["url"]
	print(os.path.join(os.path.dirname(__file__), "ipse.json"))
	custom_type_registry = load_type_registry_file(os.path.join(os.path.dirname("./"), "ipse.json"))
	substrate = SubstrateInterface(
		url=url,
		ss58_format=42,
		type_registry_preset=None,  # 这个一定要是None
		type_registry=custom_type_registry
	)

	now = substrate.query(
		module='System',
		storage_function='BlockNumber',
		params=[]
	)
	print("now:", now)

	numeric_id = config['account_id']
	mnemonic = config['account_id_to_secret_phrase'][numeric_id]
	keypair = Keypair.create_from_mnemonic(mnemonic=mnemonic)
	address = keypair.ss58_address

	history= substrate.query(
		module='PoC',
		storage_function='History',
		params=[address]
	)

	reward = 0
	if history is None:
		print("没有奖励记录(没有挖矿记录)!")
	else:
		reward_info = history.value["history"]
		print(reward_info)
		for info in reward_info[::-1]:
			if int(str(now)) - info["col1"] > DAY:
				break
			reward += info["col2"]
			print(info)
		print("过去24小时获得的总奖励是:{0}".format(reward / DOLLARS))
	email_server("过去24小时获得的总奖励是:{0}".format(reward / DOLLARS))






def check_on_chain():
	config = get_config()
	url = config["url"]
	numeric_id = config['account_id']
	official_url = config["official_url"]
	mnemonic = config['account_id_to_secret_phrase'][numeric_id]
	print(os.path.join(os.path.dirname(__file__), "ipse.json"))
	custom_type_registry = load_type_registry_file(os.path.join(os.path.dirname("./"), "ipse.json"))
	# print(custom_type_registry)

	substrate = SubstrateInterface(
		url=url,
		ss58_format=42,
		type_registry_preset= None,  # 这个一定要是None
		type_registry=custom_type_registry
	)

	official_substrate = None
	try:
		official_substrate = SubstrateInterface(
			url=official_url,
			ss58_format=42,
			type_registry_preset=None, # 这个一定要是None
			type_registry=custom_type_registry,
			# use_remote_preset= True  # 在本地获取不是远程获取

		)
	except Exception as e:
		print("官方节点不存在或是地址不正确!", e)

	keypair = Keypair.create_from_mnemonic(mnemonic=mnemonic)
	address = keypair.ss58_address

	machine_info = substrate.query(
		module='PocStaking',
		storage_function='DiskOf',
		params=[address]
	)
	staking_info = substrate.query(
		module='PocStaking',
		storage_function='StakingInfoOf',
		params=[address]
	)

	history = substrate.query(
		module='PoC',
		storage_function='History',
		params=[address]
	)

	now = substrate.query(
		module='System',
		storage_function='BlockNumber',
		params=[]
	)
	print("now:", now)

	ProbabilityDeviationValue = substrate.get_metadata_constant("PoC", "ProbabilityDeviationValue")
	ProbabilityDeviationValue = int(ProbabilityDeviationValue.value["value"], 16) / 100
	print("ProbabilityDeviationValue:", ProbabilityDeviationValue)
	net_power = substrate.query(
		module='PoC',
		storage_function='NetPower',
		params=[]
	)

	Account = substrate.query(
		module='System',
		storage_function='Account',
		params=[address]
	)

	print("Account:", Account)

	price = substrate.query(
		module='PoC',
		storage_function='CapacityPrice',
		params=[]
	)

	price = str(price)

	info = ''

	# 检查账户是否存在
	if not Account:
		print("账户不存在！")
		info += '账户不存在;'
	else:
		free_amount = Account.value["data"]["free"]
		print("账户自由余额是: {0}".format(free_amount / DOLLARS))
		# print(type(free_amount))
		if free_amount < 50 * DOLLARS:
			print("账户余额不足， 请及时充值!")
			info += "账户余额不足， 请及时充值;"

	# 检查机器是否已经注册
	if not machine_info:
		print("机器不存在(没有注册)!")
		info += "机器不存在(没有注册);"
	else:

		update_time = machine_info.value["update_time"]
		print("update_time:", update_time)
		plot_size = machine_info.value["plot_size"]
		print("miner_power:{0} Gib".format(int(str(plot_size)) / Gib))
		print("net_power:{0} Gib".format(int(str(net_power)) / Gib))

		miner_mining_num = 0
		last_mining_time = update_time
		if history:
			miner_mining_num = history.value["total_num"]
		print("miner_mining_num:", miner_mining_num)

		# todo 这个应该是根据上一次出块时间算
		net_mining_num = (int(str(now)) - update_time) // 2
		print("net_mining_num:", net_mining_num)

		numeric_id_on_chain = machine_info.value['numeric_id']
		# 检查p盘id是否一致
		if numeric_id_on_chain != numeric_id:
			print("p盘id与链上不一致! 链上id是:{0}, 配置文件id是:{1}". format(numeric_id_on_chain, numeric_id))
			info += "p盘id与链上不一致! 链上id是:{0}, 配置文件id是:{1};". format(numeric_id_on_chain, numeric_id)
		# 检查是否停止挖矿
		if machine_info.value["is_stop"]:
			print("机器已经停止挖矿!")
			info += "机器已经停止挖矿;"
		# 检查抵押是否足够
		miner_should_staking_amount = (plot_size // Gib) * int(price)
		net_should_staking_total_amount = (int(str(net_power)) // Gib) * int(price)

		# 检查挖矿概率是否存在严重偏离
		staking_amount = staking_info.value["total_staking"]
		if miner_should_staking_amount > staking_amount:
			print("抵押不够！ 应该追加抵押金额:{0}".format((miner_should_staking_amount - staking_amount) / DOLLARS))
			info += "抵押不够！ 应该追加抵押金额:{0};".format((miner_should_staking_amount - staking_amount) / DOLLARS)
		if miner_mining_num * net_should_staking_total_amount > (1 + ProbabilityDeviationValue) * (net_mining_num * miner_should_staking_amount):
			info += "挖矿概率偏高;"
			print("挖矿概率偏高!")

		if (1 - ProbabilityDeviationValue) * net_mining_num * miner_should_staking_amount > miner_mining_num * net_should_staking_total_amount:
			print("挖矿概率偏小!")
			info += "挖矿概率偏小;"

		# 检查是否与官方节点同步
		if official_substrate:
			official_now = official_substrate.query(
				module='System',
				storage_function='BlockNumber',
				params=[]
			)

			if int(str(official_now)) >= int(str(now)) and int(str(official_now)) - int(str(now)) >= 100:
				print("你的节点同步过于滞后， 请检查同步!")
				info += "你的节点同步过于滞后， 请检查同步;"
	if info:
		email_server(info)
		# todo 获取最终区块


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
		# 每一个小时检查一次（检查链上信息)
		schedule.every(1).hours.do(check_on_chain)
		# 查询24小时挖矿奖励所得
		schedule.every(1).days(1).do(get_reward_info)

		while True:
			schedule.run_pending()












