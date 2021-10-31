# Init (suppose you have finished plot)
> The following two option are the same, and you can choose one or the other.
## Option 1
* `mkdir ipse2.0-mining && cd ipse2.0-mining && wget -nc https://github.com/IPSE-TEAM/ipse2.0-mining/releases/download/v3.4.0/update_config`
> If you're not a developer，you can choose this option

## Option 2
* `git clone https://github.com/IPSE-TEAM/ipse2.0-mining.git && cd ipse2.0-mining && cargo build --release && mv ./target/release/poc-mining ./`
> If you're a developer，you can choose this option. 

# Update Config
```buildoutcfg
# command
chmod 777 update_config
./update_config
```
After the above steps have been completed， and you can find `miners_config.yaml` in the current folder. Please modify it next. （tip: the following is the default configuration, and you should use own configuration. )
```buildoutcfg
- {host: localhost, # remote server id
     account_id: 10717349404514113857, # plot id
     phase: cash mixture tongue cry roof glare monkey island unfair brown spirit inflict, # your secret key
     miner_proportion: 20,  # how much proportion that the miner should get in a rewad.
     url: 'ws://localhost:9944',  # synchronization node 
     plot_size: 50, # plot size (The unit is Gib).
     miner_reward_dest: 5FHb1AEeNui5ANvyT368dECmNEJeouLeeZ6a9z8GTvxPLaVs, # Miner income address
     plot_path: '/data/test_data',  # where is the plot file on your computer.
     max_deadline_value: 5000,  # The maximum number of the deadline allowed.
  }
```
```buildoutcfg
# command
./update_config
``` 


in the folder `localhost`, you find the another folder that named with the plot id, and into it.
***

# Start Mine

Now, you can find the file `command.txt` in current folder. 
```buildoutcfg
# command
cat command.txt
```
There are two command lines, and you can use them to stop or start mine.

* start mine
```buildoutcfg
/home/transxask/Desktop/ipse2.0-mining/localhost/10717349404514113857/supervision-10717349404514113857 --mining /home/wjy/Desktop/ipse2.0-mining/localhost/10717349404514113857/poc-mining-10717349404514113857 --log-max-size 10
```
* stop mine.
```buildoutcfg
/home/transxask/Desktop/ipse2.0-mining/localhost/10717349404514113857/supervision-10717349404514113857 --mining /home/wjy/Desktop/ipse2.0-mining/localhost/10717349404514113857/poc-mining-10717349404514113857 --log-max-size 10 --stop
```




