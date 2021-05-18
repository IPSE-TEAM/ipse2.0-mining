import time
import getopt
import sys
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

def get_config():
    with open("config.yaml", "r", encoding="utf-8") as yaml_r:
        result = yaml_r.read()
        x = yaml.load(result)
        return x


def check_on_chain():
	config = get_config()
	url = config["url"]
	substrate = SubstrateInterface(
		url=url,
		ss58_format=42,
		type_registry_preset='substrate-node-template'
	)


check_on_chain()