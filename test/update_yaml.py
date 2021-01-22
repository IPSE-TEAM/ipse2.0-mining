import yaml
import os

def get_old_yaml():
    with open("config.yaml", "r", encoding="utf-8") as yaml_r:
        result = yaml_r.read()
        x = yaml.load(result, yaml.FullLoader)

        return x


# 整理文件
def folder(account_id, host):
    pass
    # 创建以主机名为名称的文件夹
    os.system("mkdir {0}".format(host))
    # 创建以id为文件名的文件夹
    os.system("mkdir ./{0}/{1}".format(host, account_id))
    # 把挖矿软件复制到文件夹中
    os.system("cp poc-mining ./{0}/{1}/poc-mining-{2}".format(host, account_id, account_id))
    # 把监控脚本复制到文件夹中
    os.system("cp supervision.py ./{0}/{1}/supervision-{2}.py".format(host, account_id, account_id))
    # 把配置文件复制到文件夹中
    os.system("cp new_config.yaml ./{0}/{1}/config.yaml".format(host, account_id))
    # python3 supervision.py --mining poc-mining --log-max-size 10
    # 压缩文件
    os.system("tar czvf {0}.tar {1}".format(host, host))
    # 删除yaml
    os.system("rm new_config.yaml")


def update_yaml(old_yaml, miner):
    account_id = miner["account_id"]
    plot_size = miner["plot_size"]
    url = miner["url"]
    reward_dest = miner["miner_reward_dest"]
    plot_path = miner["plot_path"]
    phase = miner["phase"]
    miner_proportion =miner["miner_proportion"]
    host = miner["host"]

    old_yaml["account_id"] = account_id
    old_yaml["plot_size"] = plot_size
    old_yaml["miner_proportion"] = miner_proportion
    old_yaml["account_id_to_secret_phrase"] = []
    old_yaml["account_id_to_secret_phrase"] = {account_id: phase}
    old_yaml["plot_dirs"] = [plot_path+str(account_id)]
    old_yaml["url"] = url
    old_yaml["miner_reward_dest"] = reward_dest
    old_yaml["account_id_to_target_deadline"] = []
    old_yaml["account_id_to_target_deadline"] = {account_id: 18446744073709551615}


    with open("new_config.yaml", "w", encoding="utf-8") as yaml_w:
        yaml.dump(old_yaml, yaml_w)
    folder(account_id, host)


def get_miners_yaml():
    with open("miners_yaml.yaml", "r", encoding="utf-8") as yaml_r:
        result = yaml_r.read()
        x = yaml.load(result, yaml.FullLoader)
        print(type(x))
        return x


def main():

    old_yaml = get_old_yaml()  # 获取旧的配置文件
    miners = get_miners_yaml()["miners"]  # 获取所有矿工的配置信息
    for miner in miners:
        print(miner)
        update_yaml(old_yaml, miner)

if __name__ == "__main__":
    main()

