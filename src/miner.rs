use crate::com::api::MiningInfoResponse as MiningInfo;
use crate::config::Cfg;
use crate::cpu_worker::create_cpu_worker_task;
use crate::future::interval::Interval;
use std::convert::TryInto;
use std::time::Instant;
#[cfg(feature = "opencl")]
use crate::gpu_worker::create_gpu_worker_task;
#[cfg(feature = "opencl")]
use crate::gpu_worker_async::create_gpu_worker_task_async;
#[cfg(feature = "opencl")]
use crate::ocl::GpuBuffer;
#[cfg(feature = "opencl")]
use crate::ocl::GpuContext;
use crate::plot::{Plot, SCOOP_SIZE};
use crate::poc_hashing;
use crate::reader::Reader;
use crate::requests::RequestHandler;
use crate::utils::{get_device_id, new_thread_pool};
use crossbeam_channel;
use filetime::FileTime;
use futures::sync::mpsc;

#[cfg(feature = "opencl")]
use ocl_core::Mem;
use std::cmp::{max, min};
use std::collections::HashMap;
use std::fs::read_dir;
use std::path::PathBuf;
use std::process;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::u64;
use stopwatch::Stopwatch;
use tokio::prelude::*;
use tokio::runtime::TaskExecutor;
use tokio::clock::now;

pub const MiningExpire: u64 = 2;


pub struct Miner {
    reader: Reader,
    request_handler: RequestHandler,
    rx_nonce_data: mpsc::Receiver<NonceData>,
    target_deadline: u64,
    account_id_to_target_deadline: HashMap<u64, u64>,
    // Payee: String,
    state: Arc<Mutex<State>>,
    reader_task_count: usize,
    block_duration: u64,
    executor: TaskExecutor,
    wakeup_after: i64,

}

pub struct State {
    generation_signature_bytes: [u8; 32],
    height: u64,
    block: u64,
    account_id_to_best_deadline: HashMap<u64, u64>,
    server_target_deadline: u64,
    base_target: u64,
    sw: Stopwatch,
    scanning: bool,
    processed_reader_tasks: usize,
    scoop: u32,
    first: bool,
    outage: bool,
    // 每次获取数据的时间（挖矿开始)
    pub start_time: Instant,
    // 每次挖矿的最小值
    pub min_deadline: u64,
    // 本周期发送挖矿请求的次数
    pub mining_num: u32,
    pub is_get: bool,
    pub nonce_data: NonceData,

    pub max_deadline_value: u64,
    pub deadline : u64,
}

impl State where {
    fn new(max_deadline_value: u64) -> Self {
        Self {
            height: 0,
            block: 0,
            scoop: 0,
            account_id_to_best_deadline: HashMap::new(),
            server_target_deadline: u64::MAX,
            base_target: 1,
            processed_reader_tasks: 0,
            sw: Stopwatch::new(),
            generation_signature_bytes: [0; 32],
            scanning: false,
            first: true,
            outage: false,
            start_time: now(),
            min_deadline: u64::max_value(),
            mining_num: 0u32,
            is_get: false,
            max_deadline_value: max_deadline_value,
            nonce_data: NonceData::default(),
            deadline: u64::MAX,
        }
    }


    fn update_mining_info(&mut self, mining_info: &MiningInfo) {
        self.is_get = true;
        self.min_deadline = u64::max_value();
        self.start_time = now();
        self.mining_num = 0u32;
        for best_deadlines in self.account_id_to_best_deadline.values_mut() {
            *best_deadlines = u64::MAX;
        }

        self.height = mining_info.height;
        self.block += 1;
        self.base_target = mining_info.base_target;
        self.server_target_deadline = mining_info.target_deadline;

        self.generation_signature_bytes = mining_info.generation_signature.clone();
        let scoop =
            poc_hashing::calculate_scoop(mining_info.height, &self.generation_signature_bytes);
        info!(
            "{: <80}",
            format!("new block: height={}, scoop={}", mining_info.height, scoop)
        );
        self.scoop = scoop;

        self.sw.restart();
        self.processed_reader_tasks = 0;
        self.scanning = true;
        self.nonce_data = NonceData::default();
        self.deadline = u64::MAX;

    }
}

pub struct NonceData {
    pub height: u64,
    pub block: u64,
    pub base_target: u64,
    pub deadline: u64,
    pub nonce: u64,
    pub reader_task_processed: bool,
    pub account_id: u64,
}

impl Default for NonceData {
    fn default() -> Self {
        Self {
            height: 0,
            block: 0,
            base_target: u64::MAX,
            deadline: u64::MAX,
            nonce: u64::MAX,
            reader_task_processed: false,
            account_id: u64::MAX,

        }
    }
}

pub trait Buffer {
    fn get_buffer(&mut self) -> Arc<Mutex<Vec<u8>>>;
    fn get_buffer_for_writing(&mut self) -> Arc<Mutex<Vec<u8>>>;
    #[cfg(feature = "opencl")]
    fn get_gpu_buffers(&self) -> Option<&GpuBuffer>;
    #[cfg(feature = "opencl")]
    fn get_gpu_data(&self) -> Option<Mem>;
    fn unmap(&self);
    fn get_id(&self) -> usize;
}

pub struct CpuBuffer {
    data: Arc<Mutex<Vec<u8>>>,
}

impl CpuBuffer {
    pub fn new(buffer_size: usize) -> Self {
        let pointer = aligned_alloc::aligned_alloc(buffer_size, page_size::get());
        let data: Vec<u8>;
        unsafe {
            data = Vec::from_raw_parts(pointer as *mut u8, buffer_size, buffer_size);
        }
        CpuBuffer {
            data: Arc::new(Mutex::new(data)),
        }
    }
}

impl Buffer for CpuBuffer {
    fn get_buffer(&mut self) -> Arc<Mutex<Vec<u8>>> {
        self.data.clone()
    }
    fn get_buffer_for_writing(&mut self) -> Arc<Mutex<Vec<u8>>> {
        self.data.clone()
    }
    #[cfg(feature = "opencl")]
    fn get_gpu_buffers(&self) -> Option<&GpuBuffer> {
        None
    }
    #[cfg(feature = "opencl")]
    fn get_gpu_data(&self) -> Option<Mem> {
        None
    }
    fn unmap(&self) {}
    fn get_id(&self) -> usize {
        0
    }
}

fn scan_plots(
    plot_dirs: &[PathBuf],
    use_direct_io: bool,
    dummy: bool,
) -> (HashMap<String, Arc<Vec<Mutex<Plot>>>>, u64) {
    let mut drive_id_to_plots: HashMap<String, Vec<Mutex<Plot>>> = HashMap::new();
    let mut global_capacity: u64 = 0;

    for plot_dir in plot_dirs {
        let mut num_plots = 0;
        let mut local_capacity: u64 = 0;
        for file in read_dir(plot_dir).unwrap() {
            let file = &file.unwrap().path();

            if let Ok(p) = Plot::new(file, use_direct_io, dummy) {
                let drive_id = get_device_id(&file.to_str().unwrap().to_string());
                let plots = drive_id_to_plots.entry(drive_id).or_insert(Vec::new());

                local_capacity += p.meta.nonces as u64;
                plots.push(Mutex::new(p));
                num_plots += 1;
            }
        }

        info!(
            "path={}, files={}, size={:.4} TiB",
            plot_dir.to_str().unwrap(),
            num_plots,
            local_capacity as f64 / 4.0 / 1024.0 / 1024.0
        );

        global_capacity += local_capacity;
        if num_plots == 0 {
            warn!("no plots in {}", plot_dir.to_str().unwrap());
        }
    }

    // sort plots by filetime and get them into an arc
    let drive_id_to_plots: HashMap<String, Arc<Vec<Mutex<Plot>>>> = drive_id_to_plots
        .drain()
        .map(|(drive_id, mut plots)| {
            plots.sort_by_key(|p| {
                let m = p.lock().unwrap().fh.metadata().unwrap();
                -FileTime::from_last_modification_time(&m).unix_seconds()
            });
            (drive_id, Arc::new(plots))
        })
        .collect();

    info!(
        "plot files loaded: total drives={}, total capacity={:.4} TiB",
        drive_id_to_plots.len(),
        global_capacity as f64 / 4.0 / 1024.0 / 1024.0
    );

    if drive_id_to_plots.len() == 0 {
            panic!("err: you do not have any plots, or plot path is wrong! please check your config.");
        }

    (drive_id_to_plots, global_capacity * 64)
}

impl Miner {
    pub fn new(cfg: Cfg, executor: TaskExecutor) -> Miner {
        let (drive_id_to_plots, total_size) =
            scan_plots(&cfg.plot_dirs, cfg.hdd_use_direct_io, cfg.benchmark_cpu());

        if drive_id_to_plots.len() == 0 {
            panic!("err: you do not have any plots, or plot path is wrong! please check your config.");
        }
        let cpu_threads = cfg.cpu_threads;
        let cpu_worker_task_count = cfg.cpu_worker_task_count;

        let cpu_buffer_count = cpu_worker_task_count
            + if cpu_worker_task_count > 0 {
                cpu_threads
            } else {
                0
            };

        let reader_thread_count = if cfg.hdd_reader_thread_count == 0 {
            drive_id_to_plots.len()
        } else {
            cfg.hdd_reader_thread_count
        };

        #[cfg(feature = "opencl")]
        let gpu_worker_task_count = cfg.gpu_worker_task_count;
        #[cfg(feature = "opencl")]
        let gpu_threads = cfg.gpu_threads;
        #[cfg(feature = "opencl")]
        let gpu_buffer_count = if gpu_worker_task_count > 0 {
            if cfg.gpu_async {
                gpu_worker_task_count + 2 * gpu_threads
            } else {
                gpu_worker_task_count + gpu_threads
            }
        } else {
            0
        };
        #[cfg(feature = "opencl")]
        {
            info!(
                "reader-threads={}, CPU-threads={}, GPU-threads={}",
                reader_thread_count, cpu_threads, gpu_threads,
            );

            info!(
                "CPU-buffer={}(+{}), GPU-buffer={}(+{})",
                cpu_worker_task_count,
                if cpu_worker_task_count > 0 {
                    cpu_threads
                } else {
                    0
                },
                gpu_worker_task_count,
                if gpu_worker_task_count > 0 {
                    if cfg.gpu_async {
                        2 * gpu_threads
                    } else {
                        gpu_threads
                    }
                } else {
                    0
                }
            );

            {
                if cpu_threads * cpu_worker_task_count + gpu_threads * gpu_worker_task_count == 0 {
                    error!("CPU, GPU: no active workers. Check thread and task configuration. Shutting down...");
                    process::exit(0);
                }
            }
        }

        #[cfg(not(feature = "opencl"))]
        {
            info!(
                "reader-threads={} CPU-threads={}",
                reader_thread_count, cpu_threads
            );
            info!("CPU-buffer={}(+{})", cpu_worker_task_count, cpu_threads);
            {
                if cpu_threads * cpu_worker_task_count == 0 {
                    error!(
                    "CPU: no active workers. Check thread and task configuration. Shutting down..."
                );
                    process::exit(0);
                }
            }
        }

        #[cfg(not(feature = "opencl"))]
        let buffer_count = cpu_buffer_count;
        #[cfg(feature = "opencl")]
        let buffer_count = cpu_buffer_count + gpu_buffer_count;
        let buffer_size_cpu = cfg.cpu_nonces_per_cache * SCOOP_SIZE as usize;
        let (tx_empty_buffers, rx_empty_buffers) =
            crossbeam_channel::bounded(buffer_count as usize);
        let (tx_read_replies_cpu, rx_read_replies_cpu) =
            crossbeam_channel::bounded(cpu_buffer_count);

        #[cfg(feature = "opencl")]
        let mut tx_read_replies_gpu = Vec::new();
        #[cfg(feature = "opencl")]
        let mut rx_read_replies_gpu = Vec::new();
        #[cfg(feature = "opencl")]
        let mut gpu_contexts = Vec::new();
        #[cfg(feature = "opencl")]
        {
            for _ in 0..gpu_threads {
                let (tx, rx) = crossbeam_channel::unbounded();
                tx_read_replies_gpu.push(tx);
                rx_read_replies_gpu.push(rx);
            }

            for _ in 0..gpu_threads {
                gpu_contexts.push(Arc::new(GpuContext::new(
                    cfg.gpu_platform,
                    cfg.gpu_device,
                    cfg.gpu_nonces_per_cache,
                    if cfg.benchmark_io() {
                        false
                    } else {
                        cfg.gpu_mem_mapping
                    },
                )));
            }
        }

        for _ in 0..cpu_buffer_count {
            let cpu_buffer = CpuBuffer::new(buffer_size_cpu);
            tx_empty_buffers
                .send(Box::new(cpu_buffer) as Box<Buffer + Send>)
                .unwrap();
        }

        #[cfg(feature = "opencl")]
        for (i, context) in gpu_contexts.iter().enumerate() {
            for _ in 0..(gpu_buffer_count / gpu_threads
                + if i == 0 {
                    gpu_buffer_count % gpu_threads
                } else {
                    0
                })
            {
                let gpu_buffer = GpuBuffer::new(&context.clone(), i + 1);
                tx_empty_buffers
                    .send(Box::new(gpu_buffer) as Box<Buffer + Send>)
                    .unwrap();
            }
        }


        // 创建通道
        let (tx_nonce_data, rx_nonce_data) = mpsc::channel(buffer_count);

        thread::spawn({
            create_cpu_worker_task(
                cfg.benchmark_io(),
                new_thread_pool(cpu_threads, cfg.cpu_thread_pinning),
                rx_read_replies_cpu.clone(),
                tx_empty_buffers.clone(),
                tx_nonce_data.clone(),
            )
        });

        #[cfg(feature = "opencl")]
        for i in 0..gpu_threads {
            if cfg.gpu_async {
                thread::spawn({
                    create_gpu_worker_task_async(
                        cfg.benchmark_io(),
                        rx_read_replies_gpu[i].clone(),
                        tx_empty_buffers.clone(),
                        tx_nonce_data.clone(),
                        gpu_contexts[i].clone(),
                        drive_id_to_plots.len(),
                    )
                });
            } else {
                #[cfg(feature = "opencl")]
                thread::spawn({
                    create_gpu_worker_task(
                        cfg.benchmark_io(),
                        rx_read_replies_gpu[i].clone(),
                        tx_empty_buffers.clone(),
                        tx_nonce_data.clone(),
                        gpu_contexts[i].clone(),
                    )
                });
            }
        }

        #[cfg(feature = "opencl")]
        let tx_read_replies_gpu = Some(tx_read_replies_gpu);
        #[cfg(not(feature = "opencl"))]
        let tx_read_replies_gpu = None;

        Miner {
            reader_task_count: drive_id_to_plots.len(),
            reader: Reader::new(
                drive_id_to_plots,
                total_size,
                reader_thread_count,
                rx_empty_buffers,
                tx_empty_buffers,
                tx_read_replies_cpu,
                tx_read_replies_gpu,
                cfg.show_progress,
                cfg.show_drive_stats,
                cfg.cpu_thread_pinning,
                cfg.benchmark_cpu(),

            ),
            rx_nonce_data,
            target_deadline: cfg.target_deadline,
            account_id_to_target_deadline: cfg.account_id_to_target_deadline,
            // payee: cfg.payee,
            request_handler: RequestHandler::new(
                cfg.url,
                cfg.account_id_to_secret_phrase,
                cfg.timeout,
                (total_size * 4 / 1024 / 1024) as usize,
                cfg.send_proxy_details,
                cfg.additional_headers,
                executor.clone(),
                cfg.account_id,
                cfg.plot_size,
                cfg.miner_proportion,
                cfg.miner_reward_dest,
                // cfg.payee,

            ),
            state: Arc::new(Mutex::new(State::new(cfg.max_deadline_value))),
            // floor at 1s to protect servers
            block_duration: max(1000, cfg.block_duration),
            executor,
            wakeup_after: cfg.hdd_wakeup_after * 1000, // ms -> s
            // Payee: cfg.payee,
        }
    }

    pub fn run(mut self) {
        let request_handler = self.request_handler.clone();
        let total_size = self.reader.total_size;

        // TODO: this doesn't need to be arc mutex if we manage to separate
        // reader from miner so that we can simply move it
        let reader = Arc::new(Mutex::new(self.reader));

        let state = self.state.clone();

        let block_duration = self.block_duration;

        let wakeup_after = self.wakeup_after;

        static mut HEIGHT: u64 = 0;

        let interval_duration = Duration::from_millis(1000);

        self.executor.clone().spawn(
            Interval::new_interval(interval_duration)
                .for_each(move |_| {
                    let state = state.clone();
                    let reader = reader.clone();

                    request_handler.get_mining_info().then(move |mining_info| {

                        let mut state = state.lock().unwrap();

                        if state.is_get {
                            thread::sleep(Duration::from_millis(block_duration / 3));
                            state.is_get = false;
                        }

                        else {
                           thread::sleep(Duration::from_millis(block_duration / 24));
                        }


                        match mining_info {

                            Ok(mining_info) => {

                                state.first = false;
                                if state.outage {
                                    error!("{: <80}", "outage resolved.");
                                    state.outage = false;
                                }

                                if mining_info.generation_signature != state.generation_signature_bytes {

                                    if mining_info.height / MiningExpire != state.height / MiningExpire {

                                        state.update_mining_info(&mining_info);

                                        reader.lock().unwrap().start_reading(
                                            mining_info.height,
                                            state.block,
                                            mining_info.base_target,
                                            state.scoop,
                                            &Arc::new(state.generation_signature_bytes),
                                        );

                                        drop(state);

                                    }

                                    else {
                                    }



                                } else if !state.scanning
                                    && wakeup_after != 0
                                    && state.sw.elapsed_ms() > wakeup_after
                                {
                                    info!("HDD, wakeup!");
                                    reader.lock().unwrap().wakeup();
                                    state.sw.restart();
                                }

                            }

                            _ => {
//                                    let mut state = state.lock().unwrap();
                                if state.first {

                                    error!(
                                        "{: <80}",
                                        "error getting mining info, please check server config"
                                    );
                                    state.first = false;
                                    state.outage = true;
                                } else {
                                    if !state.outage {
                                        error!(
                                            "{: <80}",
                                            "error getting mining info => connection outage..."
                                        );
                                    }
                                    state.outage = true;
                                }
                            }
                        }

                        future::ok(())
                    })

                })
                .map_err(|e| panic!("interval errored: err={:?}", e)),
        );

        let target_deadline = self.target_deadline;
        let account_id_to_target_deadline = self.account_id_to_target_deadline;
        let request_handler = self.request_handler.clone();
        let state = self.state.clone();
        let reader_task_count = self.reader_task_count;

        self.executor.clone().spawn(
            self.rx_nonce_data
                .for_each(move |nonce_data| {

                    let mut state = state.lock().unwrap();

                    let end = now();

                    let len = state.height - nonce_data.height;

                    let deadline = nonce_data.deadline / nonce_data.base_target;

                    if state.height / MiningExpire == nonce_data.height / MiningExpire {
                        if deadline < state.deadline {

                            state.deadline = deadline;
                            state.nonce_data = nonce_data;
                            info!("deadline:{:?}", state.deadline);

                        }

                        state.mining_num += 1;
                        state.processed_reader_tasks += 1;



                    }

                    let mut end_num = 0u64;

                    if (total_size * 4) % (16 * 1024 * 1024) == 0 {
                        end_num = total_size * 4 / 16 / 1024 / 1024
                    }

                    else {
                        end_num = total_size * 4 / 16 / 1024 / 1024 + 1;
                    }

                    if state.mining_num == (end_num as u32) && state.height / MiningExpire == state.nonce_data.height / MiningExpire {
                        info!("%%%%%%%%%%%%%%%%%%%%%%%%%  scan plot spend time: {:?} %%%%%%%%%%%%%%%%%%%%%%%%%%",
                          (end - state.start_time) + Duration::from_millis(len * 12000));
                    }
                    // info!("end_num: {:?}, deadline: {:?}", end_num, state.deadline);

                    if state.height / MiningExpire == state.nonce_data.height / MiningExpire &&
                        state.deadline <= state.max_deadline_value && state.mining_num == (end_num as u32) {

                        info!("scan finish. deadline： {:?}, \
                        max deadline: {:?}", state.deadline, state.max_deadline_value);

                        let nonce_data = &state.nonce_data;

                            async_std::task::block_on(async {
                                request_handler.submit_nonce(
                                nonce_data.account_id,
                                nonce_data.nonce,
                                nonce_data.height, // 这个高度就是获取数据时候的高度
                                nonce_data.block, // 这个值貌似没有什么用
                                nonce_data.deadline, // deadline_unadjusted
                                state.deadline, // 提交到链上的是这个deadline
                                state.generation_signature_bytes,
                                );
                            });

                    }

                    else if state.height / MiningExpire != state.nonce_data.height / MiningExpire {
                        info!("scan overdue, can not submmit!");

                    }

                    else {
                    }


                    if state.nonce_data.reader_task_processed {

                        if state.processed_reader_tasks == reader_task_count {
                            // info!(
                            //     "{: <80}",
                            //     format!(
                            //         "round finished: roundtime={}ms, speed={:.2}MiB/s",
                            //         state.sw.elapsed_ms(),
                            //         total_size as f64 * 1000.0
                            //             / 1024.0
                            //             / 1024.0
                            //             / state.sw.elapsed_ms() as f64
                            //     )
                            // );
                            state.sw.restart();
                            // info!("%%%%%%%%%%% finished sw.restart %%%%%%%%%%%");
                            state.scanning = false;


                        }


                    }


                    Ok(())
                })
                .map_err(|e| panic!("interval errored: err={:?}", e)),
        );
    }
}
