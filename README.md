# ftcp
An tcp proxy server/client which exchange the data in temp files

通过在临时文件中交换数据来进行TCP代理的一个服务端/客户端

学校内网中有针对教学楼的防火墙导致教室电脑难以上网（

但学校内建有公共ftp服务器，因此就有了这个借助文件进行代理的项目

目前这个玩意会不停造线程，也不能及时删除临时文件，还只能用本地文件转发，停留在非常初级的试用阶段。

占用资源极大且最开始的几个链接可能完全不能用，可能是系统线程调度策略导致的。

之后会逐渐改写为基于tokio的异步代理库（当然高三了也没太多时间搞这个