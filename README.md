# ftcp
A tcp proxy server/client which exchange the data in temp files

通过在临时文件中交换数据来进行TCP代理的一个服务端/客户端

学校内网中有针对教学楼的防火墙导致教室电脑难以上网（

但学校内建有公共ftp服务器，因此就有了这个借助文件进行代理的项目

基于tokio，由于crates.io上的async_ftp不支持gbk所以魔改了一份（尚未放上文件），目前还是本地文件转发的试验版本

由于我校ftp服务器有至少0.2s的延迟所以会尽力通过高并发来提速（当然高三了也没太多时间搞这个

目前的逻辑是对每一个tcp连接开一个文件夹，文件夹内建立upload和download两个子文件夹

然后客户端向upload写入并轮询download，服务端反之

一个tcp连接中的每个包被作为一个文件转发
