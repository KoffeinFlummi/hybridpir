package de.tu_darmstadt.cs.encrypto.hybridpir;

import java.util.Arrays;

import androidx.appcompat.app.AppCompatActivity;

import android.os.Bundle;
import android.widget.TextView;

public class MainActivity extends AppCompatActivity {

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        setContentView(R.layout.activity_main);

        RustInterface i = new RustInterface();

        String targets[] = {
            "192.168.178.20:7000",
            "192.168.178.20:7001"
        };

        byte[] query = i.sendQuery(
            targets,
            1 << 20,
            8,
            2,
            1 << 12,
            2048,
            12,
            2,
            1 << 19
        );

        TextView tv = (TextView)findViewById(R.id.textView);
        tv.setText(Arrays.toString(query));
    }

    static {
        System.loadLibrary("hybridpir");
    }
}
