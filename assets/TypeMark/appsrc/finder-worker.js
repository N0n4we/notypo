
var path = require("path");

var supportTextBundle = false;
var supportTextPack = false;
var defaultExtension = "md";

var _MAX_COUNT = 600;
var _MAX_ITER = 4000;

onmessage = function(e) {
	var command = e.data[0],
		args = e.data.slice(1);

	api[command].apply(null, args);
}

var fsWatcher;

var unwatch = function(){
	if(fsWatcher){
		fsWatcher.close();
		fsWatcher._handle = null;
		fsWatcher = null;
	}
}

var watchFolder = function(folder) {
	if(fsWatcher){
		fsWatcher.close();
		fsWatcher._handle = null;
		fsWatcher = null;
	}
	if(!folder) return;

	if(process.platform == "linux") {
		watchFolderForLinux(folder);
	} else {
		watchFolderForWin(folder);
	}
}

var handledList = [];
var clearHandledListTimer;
var scheduleClear = function(){
	clearHandledListTimer && clearTimeout(clearHandledListTimer)
	clearHandledListTimer = setTimeout(() => {
		clearHandledListTimer = null;
		handledList = [];
	}, 100);
}

var watchFolderForWin = function(folder) {
	// watch use fs.watch with recursive option
	try {
		const fs = require("fs-extra");
		fsWatcher = fs.watch(folder, {
			persistent: false,
			recursive: true
		});
	
		fsWatcher.on("error", function() {
			unwatch();
			watchFolderForLinux();
		});
	
		fsWatcher.on("change", async function(eventType, filename) {
			if(eventType == "change") return;
			
			var filepath = path.join(folder, filename);
	
			if(/(^|\/)\.[^\.]+$/.exec(filepath)) return;
			var name = path.basename(filepath);
			if(!canOpenByTypora(name)) return;
			
			var attr;
			try {
				var	stats = await fs.stat(filepath);
				attr = statsToAttr(filepath, stats);
			} catch(e){}
	
			if(!attr || attr.shouldIgnore) {
				postMessage({
					"type": "removeInitFiles",
					"detail": [filepath]
				});
				return;
			} else {
				addFileOrFolder(filepath, attr);
				scheduleClear();
			}
		});
	} catch(e) {
		console.warn(e);
	}
}

function addFileOrFolder(filepath, attr){
	if(!attr || attr.shouldIgnore) return;

	if(supportTextBundle) {
		if(/\.textbundle[\/\\]/.exec(filepath)) {
			filepath = filepath.replace(/(\.textbundle[\/\\]).+/, "$1");
		}
	}

	if(attr.isFile){
		if(handledList.indexOf(filepath) > -1) return;

		postMessage({
			"type": "addInitFiles",
			"detail": [[filepath], [path.basename(filepath)], [attr.mtime]]
		});
	}

	if(attr.isDir){
		var pathArray = [], fileArray = [], modifiedDateArray = [];

		if(handledList.indexOf(filepath) > -1) return;
		
		walk(filepath, function(filepath, filename, attr){
			pathArray.push(filepath);
			fileArray.push(filename);
			modifiedDateArray.push(attr.mtime);
		}).then(function(){
			postMessage({
				"type": "addInitFiles",
				"detail": [pathArray, fileArray, modifiedDateArray]
			})
		});
	}
}

var watchFolderForLinux = function(folder) {
	// watch use chokidar
	
	console.debug(`watch folder ${folder}`)
	const fs = require("fs-extra");
	const chokidar = require("chokidar");
	fsWatcher = chokidar.watch(folder, {
		ignored: function(filepath, stats){
			if(/(^|\/)\.[^\.]+($|\/)/.exec(filepath.substr(folder.length))) return true;
			if(!stats) return;
			var attr = statsToAttr(filepath, stats);
			return !attr || attr.shouldIgnore;
		},
		ignoreInitial: true,
		binaryInterval: 30000,
		interval: 500,
		depth: 4,
		ignorePermissionErrors: true,
		followSymlinks: false
	});
	fsWatcher.on("add", async function(filepath, stats){
		if(/(^|\/)\.[^\.]+$/.exec(filepath)) return;
		var name = path.basename(filepath);
		if(!canOpenByTypora(name)) return;
		if(!stats) {
			stats = await fs.stat(filepath);
		}
		var attr = statsToAttr(filepath, stats);
		if(!attr || attr.shouldIgnore || attr.isDir) return;
		addFileOrFolder(filepath, attr);
		scheduleClear();
	}).on("unlink", function(filepath){
		postMessage({
			"type": "removeInitFiles",
			"detail": [filepath]
		});
	}).on("addDir", async function(folder, stats){
		if(/(^|\/)\.[^\.]+$/.exec(folder)) return;
		if(!stats) {
			stats = await fs.stat(folder);
		}
		var attr = statsToAttr(folder, stats);
		if(!attr || attr.shouldIgnore) return;
		addFileOrFolder(folder, attr);
		scheduleClear();
	}).on("unlinkDir", function(filepath){
		postMessage({
			"type": "removeInitFiles",
			"detail": [filepath]
		});
	});
}

var statsToAttr = function(filepath, stats){
	if(!stats) return null;

	var attr = {};
	attr.isDir = stats.isDirectory();
	attr.isFile = stats.isFile();
	attr.mtime = stats.mtime;
	attr.birthtime = stats.birthtime;
	attr.size = Number(stats.size)/1000;

	var name = path.basename(filepath);

	if(/\.asar[/\\]*$/i.exec(name)) {
			attr.isFile = true;
			attr.isDir = false;
			attr.shouldIgnore = true;
	}

	if(attr.isDir){
		attr.shouldIgnore = (name == "node_modules") ;
	} else if(attr.isFile){
		attr.shouldIgnore = attr.shouldIgnore || !canOpenByTypora(name) || attr.size > 1000;
	}

	if(supportTextBundle && /\.textbundle$/i.exec(name)) {
		attr.isDir = false;
		attr.isFile = true;
		attr.shouldIgnore = false;
	}

	return attr;
}

var statWrapper = function(fs, filepath, callback) {
    var attr = {},
        name = path.basename(filepath);

    if(name && name[0] == "."){
        attr.shouldIgnore = true;
        return callback(attr);
    }
    fs.stat(filepath, function(error, stats){
        if(error || !stats){
            return callback(null);
        }

        attr = statsToAttr(filepath, stats);
        callback(attr);
    });
};

var SupportedFiles =["md", "markdown", 'mmd', "mkd", "mdwn", 'mdown', 'mdx', "", "mdtxt", "mdtext", "apib", 'rmarkdown', 'rmd', 'qmd', "txt", "text"];

if(supportTextBundle) {
	SupportedFiles.push("textbundle");
}

if(supportTextPack) {
	SupportedFiles.push("textpack");
}

var canOpenByTypora = function (filename) {
	if(filename[0] == ".") return false;
	var ext = path.extname(filename).replace(/^\./, '');
	if(~SupportedFiles.indexOf(ext.toLowerCase())){
		return true;
	}
}

var walk = function(rootPath, onFile, onDir){
	var fs = require("fs-plus");
	var shouldBreak;

	function walkNode(filename, parent){
		return new Promise(function(resolve, reject){
			if(filename[0] == "." || filename == "node_modules" || shouldBreak){
				return resolve();
			}

			var childPath = path.join(parent, filename);

			statWrapper(fs, childPath, function(attr){
				if(!attr || attr.shouldIgnore) return resolve();

				if(attr.isDir){
					if(onDir && onDir(childPath, filename, attr) === false) {
						shouldBreak = true;
						return resolve();
					}

					walkFolder(childPath).then(resolve);
					return;
				}

				if(attr.isFile){
					if(onFile && onFile(childPath, filename, attr) === false){
						shouldBreak = true;
					}
				}

				return resolve();
			});
		});
	}

	function walkFolder(folder){
		return new Promise(function(resolve, reject){
			fs.readdir(folder, function(err, files){
				if(err) return resolve();

				Promise.all(files.map(function(filename){
					return walkNode(filename, folder);
				})).then(resolve);
			});
		});
	}

	return new Promise(function(resolve, reject){
		if(!rootPath) return resolve();

		walkFolder(rootPath).then(resolve);
	});
}

var listAllFiles = function(rootPath){
	workspace_ = rootPath;
	var pathArray = [], fileArray = [], modifiedDateArray = [];
	var fetchedCount = 0, iterCount = 0;
	var needBackEndQuery = false;

	watchFolder(rootPath);
	
	if(!rootPath){
		postMessage({
			"type": "initFileCache",
			"detail": [pathArray, fileArray, modifiedDateArray, needBackEndQuery ? _MAX_COUNT + 2 : fetchedCount]
		});
		return;
	}

	walk(rootPath, function(filepath, filename, attr){
		iterCount++;
		pathArray.push(filepath);
		fileArray.push(filename);
		modifiedDateArray.push(attr.mtime);
		fetchedCount++;
		if(fetchedCount > _MAX_COUNT || iterCount > _MAX_ITER){
			needBackEndQuery = true;
			return false
		}
	}).then(function(){
		postMessage({
			"type": "initFileCache",
			"detail": [pathArray, fileArray, modifiedDateArray, needBackEndQuery ? fetchedCount + 2 : fetchedCount]
		})
	});
}

var curQueryKey_;
var _MAX_QUERY_HIT = 100;

var quickFind = function(rootPath, key){
	if(!rootPath) return;

	curQueryKey_ = key;

	var hitCount = 0;

	var reg = new RegExp(key.replace(/[*]/g, ' ').replace(/[\-\[\]\/\{\}\(\)\*\+\?\.\\\^\$\|]/g, "\\$&").split(/\s+/).join(".*"), "i");

	walk(rootPath, function(filepath, filename, attr){
		if(!curQueryKey_) return false;
		if(curQueryKey_ != key) return false;
		if(canOpenByTypora(filename) && reg.exec(filename)){
			hitCount++;
			if(hitCount > _MAX_QUERY_HIT) return false;
			postMessage({
				"type": "queryMatch",
				"detail": [[filepath]]
			})
		}
	}, function(filepath, filename, attr){
		if(!curQueryKey_) return false;
		if(curQueryKey_ != key) return false;
	}).then(function(){
		if(curQueryKey_ == key){
			postMessage({
				"type": "queryEnd"
			});
		}
	});
}

var stopQuery = function(){
	curQueryKey_ = undefined;
}

var nodeInit = function(dirname){
	var Module = require("module");	
	var node_modules_asar = path.join(dirname, "node_modules.asar");
	Module.globalPaths.push(node_modules_asar);

	var _old_require = Module.prototype.require;
	Module.prototype.require = function(id){
		if(this.paths.indexOf(node_modules_asar) == -1) {
			this.paths.push(node_modules_asar);
		}
		return _old_require.call(this, id);
	};
}

var setSupportTextBundle = function(v, p) {
	supportTextBundle = v;
	supportTextPack = p;
}

var setDefaultExtension = function(v) {
	if(SupportedFiles.indexOf(v) == -1) {
		SupportedFiles.push(v);
	}
}

var api = {
	listAllFiles: listAllFiles,
	quickFind: quickFind,
	nodeInit: nodeInit,
	setSupportTextBundle: setSupportTextBundle,
	setDefaultExtension: setDefaultExtension
}
